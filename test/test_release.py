import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / ".github/scripts/publish-release.sh"
FAKE_GH = """#!/usr/bin/env python3
import json
import os
import sys

args = sys.argv[1:]
with open(os.environ["GH_TEST_LOG"], "a") as log:
    log.write(json.dumps(args) + "\\n")
operation = args[0] if args[0] == "api" else args[1]
if operation == os.environ.get("GH_TEST_FAIL"):
    sys.exit(1)
if operation == "api":
    print(os.environ.get("GH_TEST_RELEASES", "[]"))
"""


class ReleaseTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "dist").mkdir()
        (self.root / "bin").mkdir()
        gh = self.root / "bin/gh"
        gh.write_text(FAKE_GH)
        gh.chmod(0o755)
        self.log = self.root / "gh.log"
        self.env = {
            **os.environ,
            "PATH": f"{self.root / 'bin'}:{os.environ['PATH']}",
            "GH_REPO": "example/sectora",
            "GITHUB_REF_NAME": "v0.6.0",
            "VERSION": "0.6.0",
            "GH_TEST_LOG": str(self.log),
        }
        self.package("amd64")
        self.package("arm64")

    def package(self, arch, version="0.6.0-1", name="sectora", architecture=None):
        control = self.root / "package/DEBIAN/control"
        control.parent.mkdir(parents=True, exist_ok=True)
        control.write_text(
            f"Package: {name}\nVersion: {version}\nArchitecture: {architecture or arch}\n"
            "Maintainer: Test <test@example.com>\nDescription: Release test fixture\n"
        )
        path = self.root / f"dist/sectora_{version}_{arch}.deb"
        subprocess.run(
            ["dpkg-deb", "--build", "--root-owner-group", str(control.parents[1]), str(path)],
            check=True, capture_output=True,
        )
        return path

    def publish(self, success=True, **env):
        result = subprocess.run(
            ["bash", str(SCRIPT)], cwd=self.root, env={**self.env, **env},
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def test_publishes_both_packages_from_a_draft(self):
        calls = self.publish()
        self.assertEqual([call[:2] for call in calls[1:]], [
            ["release", "create"], ["release", "upload"], ["release", "edit"],
        ])
        self.assertIn("--draft", calls[1])
        self.assertIn("--verify-tag", calls[1])
        self.assertEqual(calls[2][3:], [
            "dist/sectora_0.6.0-1_amd64.deb", "dist/sectora_0.6.0-1_arm64.deb", "--clobber",
        ])
        self.assertIn("--draft=false", calls[3])

    def test_resumes_a_draft_without_creating_another_release(self):
        calls = self.publish(GH_TEST_RELEASES='[{"tag_name":"v0.6.0","draft":true}]')
        self.assertEqual([call[:2] for call in calls[1:]], [
            ["release", "upload"], ["release", "edit"],
        ])

    def test_leaves_published_releases_unchanged(self):
        calls = self.publish(GH_TEST_RELEASES='[{"tag_name":"v0.6.0","draft":false}]')
        self.assertEqual(len(calls), 1)

    def test_api_and_upload_failures_never_publish(self):
        for operation in ("api", "create", "upload"):
            with self.subTest(operation=operation):
                self.log.unlink(missing_ok=True)
                calls = self.publish(success=False, GH_TEST_FAIL=operation)
                self.assertEqual(calls[-1][0 if operation == "api" else 1], operation)
                self.assertNotIn(["release", "edit"], [call[:2] for call in calls])
                if operation == "api":
                    self.assertEqual(len(calls), 1)

    def test_rejects_mismatched_tag(self):
        self.assertEqual(self.publish(success=False, GITHUB_REF_NAME="v0.5.0"), [])

    def test_rejects_missing_package(self):
        (self.root / "dist/sectora_0.6.0-1_arm64.deb").unlink()
        self.assertEqual(self.publish(success=False), [])

    def test_rejects_duplicate_packages(self):
        self.package("arm64", version="0.6.0-2")
        self.assertEqual(self.publish(success=False), [])

    def test_rejects_wrong_package_metadata(self):
        original = self.root / "dist/sectora_0.6.0-1_arm64.deb"
        for metadata in ({"version": "0.5.0-1"}, {"name": "other"}, {"architecture": "amd64"}):
            with self.subTest(metadata=metadata):
                path = self.package("arm64", **metadata)
                path.replace(original)
                self.assertEqual(self.publish(success=False), [])


if __name__ == "__main__":
    unittest.main()
