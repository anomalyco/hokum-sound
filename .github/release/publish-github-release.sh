#!/usr/bin/env bash
# publish-github-release.sh <dist>: mirrors a stable release's files to its
# GitHub release HOKUM_RELEASE_TAG, byte for byte, with the release notes.
# Release runs it last, after the release service published and
# tag-release.sh tagged v<version> at the release commit; it never creates the
# tag itself. Stable releases become "Latest".
#
# Environment: HOKUM_VERSION, HOKUM_RELEASE_COMMIT, HOKUM_RELEASE_TAG,
# HOKUM_RELEASE_NOTES (the notes file), GITHUB_REPOSITORY, GH_TOKEN.
. "$(dirname "$0")/lib.sh"

dist=$(posix "${1:?Usage: publish-github-release.sh <dist>}")
repo=$GITHUB_REPOSITORY
version=${HOKUM_VERSION:?Set HOKUM_VERSION}
commit=${HOKUM_RELEASE_COMMIT:?Set HOKUM_RELEASE_COMMIT}
tag=${HOKUM_RELEASE_TAG:?Set HOKUM_RELEASE_TAG}
notes=${HOKUM_RELEASE_NOTES:?Set HOKUM_RELEASE_NOTES}
[ -f "$notes" ] || die "Add release notes at $notes."

assets=()
for label in x64 arm64; do
  for suffix in "" .deb .rpm; do assets+=("hokum-sound-$version-linux-$label$suffix"); done
done
for name in "${assets[@]}"; do [ -f "$dist/$name" ] || die "No $name in $dist."; done
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT

tagged=$(gh api "repos/$repo/git/ref/tags/$tag" --jq .object.sha)
[ "$tagged" = "$commit" ] || die "Tag $tag names $tagged, not $commit."
if ! gh release view "$tag" -R "$repo" --json name > /dev/null 2>&1; then
  gh release create "$tag" -R "$repo" --verify-tag --title "Hokum Sound $version" \
    --notes-file "$notes" --latest=true
fi
gh release view "$tag" -R "$repo" --json assets > "$temporary/release.json"
mkdir -p "$temporary/mirror"
for name in "${assets[@]}"; do
  if ! jq -e --arg name "$name" '.assets | any(.name == $name)' "$temporary/release.json" > /dev/null; then
    gh release upload "$tag" "$dist/$name" -R "$repo"
  fi
  gh release download "$tag" -R "$repo" --pattern "$name" --dir "$temporary/mirror" --clobber
  cmp -s "$dist/$name" "$temporary/mirror/$name" || die "GitHub's $name differs from the release bytes."
  rm -f "$temporary/mirror/$name"
done
echo "https://github.com/$repo/releases/tag/$tag"
