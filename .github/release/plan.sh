#!/usr/bin/env bash
# plan.sh: decides what this run builds and where it publishes, from the event
# and the ref alone, and writes the plan to $GITHUB_OUTPUT.
#
#   dev    a push to dev (or a dispatch from dev): 0.0.0-dev-<run number> on
#          the release service's dev channel, as OpenCode and Hex number dev
#          builds.
#   check  a dispatch from dev: the same build, packaged and tested,
#          published nowhere.
#   stable a dispatch from production: the crate version from Cargo.toml on
#          the stable channel. Tags never start a release; tag-release.sh tags
#          v<version> only after everything published, so the version must
#          not be tagged yet.
#
# Environment: EVENT, REF_NAME, INPUT_REF, CHANNEL, SHA, GH_TOKEN, and
# Cargo.toml of the commit in the working directory.
. "$(dirname "$0")/lib.sh"

version=$(awk '/^\[package\]/ { package = 1; next } /^\[/ { package = 0 } package && /^version = "/ { gsub(/^version = "|"$/, ""); print; exit }' Cargo.toml)
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "Stable releases need an X.Y.Z crate version, not $version."
case "$CHANNEL" in
  stable)
    if [ "$EVENT" != workflow_dispatch ] || [ "$REF_NAME" != production ] || [ "$INPUT_REF" != production ]; then
      die "Stable releases are dispatched from production for ref production."
    fi
    tag="v$version"
    if gh api "repos/$GITHUB_REPOSITORY/git/matching-refs/tags/$tag" --jq '.[].ref' | grep -qx "refs/tags/$tag"; then
      die "Hokum Sound $version is already tagged $tag; raise the version in Cargo.toml on production first."
    fi
    [ -f "docs/releases/$version.md" ] || die "Add release notes at docs/releases/$version.md first."
    release_channel=stable release_version=$version
    notes="https://github.com/$GITHUB_REPOSITORY/releases/tag/$tag"
    ;;
  dev | check)
    # A check run proves the tag job can push a v* tag with a throwaway
    # v0.0.0-test-<run> tag it deletes again.
    [ "$REF_NAME" = dev ] || die "The $CHANNEL channel runs from dev, not $REF_NAME."
    if [ "$EVENT" = workflow_dispatch ] && [ "$INPUT_REF" != dev ]; then die "The $CHANNEL channel builds ref dev, not $INPUT_REF."; fi
    # Display text only: each channel has one current release.
    release_channel=dev release_version="0.0.0-dev-$GITHUB_RUN_NUMBER" tag=""
    if [ "$CHANNEL" = check ]; then tag="v0.0.0-test-$GITHUB_RUN_NUMBER"; fi
    notes="https://github.com/$GITHUB_REPOSITORY/commit/$SHA"
    ;;
  *) die "Unknown channel $CHANNEL." ;;
esac
{
  echo "sha=$SHA"
  echo "version=$version"
  echo "channel=$CHANNEL"
  echo "service=https://anoma.ly/release"
  echo "tag=$tag"
  echo "notes-url=$notes"
  echo "release-channel=$release_channel"
  echo "release-version=$release_version"
} >> "$GITHUB_OUTPUT"
echo "Releasing Hokum Sound $version ($CHANNEL) from $SHA as $release_channel $release_version." >> "$GITHUB_STEP_SUMMARY"
