# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog][keep a changelog] and this project adheres to [Semantic Versioning][semantic versioning].

## [Unreleased]

---

## [Released]

## [0.6.0] - 2026-11-13

### Added

- Authenticate with a GitHub App installation using `app.id` and `app.private_key_path`, with automatic installation discovery and token refresh.

### Changed

- Use Rust 2024 edition, require Rust 1.97.1, and update dependencies.
- Build Debian packages on Ubuntu 22.04 (Jammy) and default local builds to `TARGET=arm64`.
- Store authorization caches in `/var/cache/sectora` by default, configurable with `cache_dir`.
- Publish releases only after both amd64 and arm64 packages pass tag, version, and package metadata checks; allow failed draft uploads to be retried.

### Removed

- Remove armhf builds and release packages. Supported package architectures are amd64 and arm64.

### Fixed

- Keep the daemon running through GitHub API failures, apply request timeouts, and fall back to existing caches when refreshes fail.
- Respect `cert_path` for GitHub HTTPS requests when `SSL_CERT_FILE` is not set.
- Restore home directory creation after Ansible installation and migrate the old malformed PAM rule.
- Resolve the SSH key helper by its absolute path so Ansible installations work when SSH omits `/usr/sbin` from `PATH`.
- Allow AppArmor-confined PAM helpers to query sectora NSS sockets, including on systems using PAM 1.7.

### Security

- Preserve host PAM authentication and account restrictions in Debian and Ansible installations, including migration of legacy sectora rules.
- Keep authorization caches private and reject untrusted paths, symlinks, foreign owners, and unsafe cache entries.
- Protect client reply sockets with a sticky directory and reject unsafe socket paths without modifying unexpected files or directories.

### Upgrade notes

- Old URL-shaped cache files are not imported or removed. Refresh from GitHub once after upgrading before relying on cached access during an outage.
- Custom cache directories must be private, owned by the daemon user, and located under trusted parents. If startup rejects an old cache or socket path, inspect its ownership and contents before changing or removing it.
- For manual installations, keep the distribution's PAM authentication and account rules, restore `@include common-auth` if previously disabled, and remove legacy sectora PAM rules as described in the [installation instructions](README.md#configure-pam-optional).
- The Debian package installs the AppArmor abstraction and reloads an active `unix-chkpwd` profile. Manual installations and container hosts need the corresponding [AppArmor setup](README.md#configure-name-service-switch).

## [0.5.0] - 2025-04-05

### Added

- Add arm64 builds and release packages.

### Changed

- Use clap for CLI parsing and shell completions, and reqwest for GitHub API requests.
- Update dependencies and Docker build tooling.

## [0.4.0] - 2021-01-22

### Changed

- Update dependencies
- Use tokio 1.0
- Allow long messages

### Fixed

- fix unknown key name issue #356

## [0.3.2] - 2020-09-12

### Changed

- Update dependencies

## [0.3.1] - 2020-08-12

### Changed

- Tweak PAM settings
- Update depnedencies

### Fixed

- Use the abusolute path for conffiles

## [0.3.0] - 2020-06-16

### Added

- Notify Systemd

### Changed

- Update the rust version
- Handle the send-back error
- Update default AuthorizedKeysCommand style
- Update depnedencies

## [0.2.0] - 2020.05.30

### Added

- Enhance rate limit information

### Fixed

- Rewrite all hyper-related methods to asynchronous ones

### Changed

- Make http client reusable
- Update some depnedencies

## [0.1.0] - 2020.05.16

### Added

- Initial release

---

<!-- Links -->

[keep a changelog]: https://keepachangelog.com/
[semantic versioning]: https://semver.org/

<!-- Versions -->

[unreleased]: https://github.com/yasuyuky/sectora/compare/v0.6.0...HEAD
[released]: https://github.com/yasuyuky/sectora/releases
[0.6.0]: https://github.com/yasuyuky/sectora/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/yasuyuky/sectora/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/yasuyuky/sectora/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/yasuyuky/sectora/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/yasuyuky/sectora/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/yasuyuky/sectora/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/yasuyuky/sectora/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/yasuyuky/sectora/releases/v0.1.0
