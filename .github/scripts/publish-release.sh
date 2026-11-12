#!/bin/bash
set -euo pipefail

test "$GITHUB_REF_NAME" = "v$VERSION"
shopt -s nullglob
assets=()
for arch in amd64 arm64; do
    packages=(dist/*_"$arch".deb)
    if [ "${#packages[@]}" -ne 1 ]; then
        echo "Expected exactly one $arch package, found ${#packages[@]}" >&2
        exit 1
    fi
    package=${packages[0]}
    test "$(dpkg-deb --field "$package" Package)" = sectora
    test "$(dpkg-deb --field "$package" Architecture)" = "$arch"
    package_version=$(dpkg-deb --field "$package" Version)
    test "${package_version%-*}" = "$VERSION"
    test "${package##*/}" = "sectora_${package_version}_${arch}.deb"
    assets+=("$package")
done

# List releases so an API failure cannot be mistaken for a missing release.
releases=$(gh api --paginate "repos/$GH_REPO/releases")
draft=$(jq -r --arg tag "$GITHUB_REF_NAME" '.[] | select(.tag_name == $tag) | .draft' <<< "$releases")
if [ "$draft" = false ]; then
    echo "Release $GITHUB_REF_NAME is already published; leaving it unchanged."
    exit 0
fi
if [ -z "$draft" ]; then
    gh release create "$GITHUB_REF_NAME" --verify-tag --draft --generate-notes \
        --title "Release $GITHUB_REF_NAME ($(date -u +'%Y.%m.%d'))"
fi

# Upload failures leave a resumable draft instead of a partial public release.
gh release upload "$GITHUB_REF_NAME" "${assets[@]}" --clobber
gh release edit "$GITHUB_REF_NAME" --draft=false --verify-tag
