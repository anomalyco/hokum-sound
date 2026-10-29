#!/usr/bin/env bash
# tag-release.sh: pushes the lightweight tag HOKUM_RELEASE_TAG at the released
# commit SHA (the checked-out HEAD) with the job's GITHUB_TOKEN, which the
# checkout left in git's config. Tags trigger nothing. A rerun accepts the tag
# when it already names SHA. A check run (HOKUM_RELEASE_TAG_TEST=1) pushes a
# throwaway tag and deletes it again. Runs git only, with hooks off.
. "$(dirname "$0")/lib.sh"

tag=$HOKUM_RELEASE_TAG
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-test-[0-9]+)?$ ]] || die "Unexpected tag $tag."
[ "$(git rev-parse HEAD)" = "$SHA" ] || die "The checkout is not the released commit $SHA."
git() { command git -c core.hooksPath=/dev/null "$@"; }

existing=$(git ls-remote origin "refs/tags/$tag" | cut -f1)
if [ -n "$existing" ]; then
  [ "$existing" = "$SHA" ] || die "Tag $tag already names $existing, not $SHA."
  echo "Tag $tag already names $SHA."
else
  git push -q origin "$SHA:refs/tags/$tag"
  [ "$(git ls-remote origin "refs/tags/$tag" | cut -f1)" = "$SHA" ] || die "Tag $tag did not land."
  echo "Tagged $SHA as $tag."
fi
if [ "${HOKUM_RELEASE_TAG_TEST:-}" = 1 ]; then
  git push -q origin ":refs/tags/$tag"
  [ -z "$(git ls-remote origin "refs/tags/$tag")" ] || die "Could not delete the test tag $tag."
  echo "Deleted the test tag $tag."
fi
