#!/bin/bash
set -euo pipefail

test_dir=$(mktemp -d)
trap 'rm -rf "$test_dir"' EXIT
chmod 1777 "$test_dir"
cat >"$test_dir/config" <<EOF
token = "TESTTOKEN"
org = "soundtribe"
endpoint = "http://json-server:3000"
cache_dir = "$test_dir/cache"
EOF

reject_startup() {
    local status=0
    timeout 3s env TMPDIR="$test_dir" SECTORA_CONFIG="$test_dir/config" \
        /usr/sbin/sectorad >"$test_dir/log" 2>&1 || status=$?
    if [ "$status" -eq 0 ] || [ "$status" -eq 124 ]; then
        cat "$test_dir/log"
        echo "Daemon accepted an untrusted socket path" >&2
        exit 1
    fi
}

mkdir -m 700 "$test_dir/outside"
ln -s "$test_dir/outside" "$test_dir/sectora"
reject_startup
test "$(stat -c %a "$test_dir/outside")" = 700
test -L "$test_dir/sectora"
rm "$test_dir/sectora"

mkdir -m 755 "$test_dir/sectora"
chown nobody "$test_dir/sectora"
reject_startup
test "$(stat -c %a "$test_dir/sectora")" = 755
test "$(stat -c %U "$test_dir/sectora")" = nobody
chown root "$test_dir/sectora"

printf keep >"$test_dir/sectorad"
reject_startup
test "$(cat "$test_dir/sectorad")" = keep
rm "$test_dir/sectorad"
ln -s "$test_dir/outside" "$test_dir/sectorad"
reject_startup
test -L "$test_dir/sectorad"
test "$(stat -c %a "$test_dir/outside")" = 700

# A different local user cannot remove a root-owned reply socket entry.
test "$(stat -c %a /tmp/sectora)" = 1777
entry=$(mktemp /tmp/sectora/startup-test.XXXXXX)
if runuser -u nobody -- rm "$entry"; then
    echo "Another user removed a reply socket entry" >&2
    exit 1
fi
rm "$entry"
