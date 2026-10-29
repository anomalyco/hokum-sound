#!/usr/bin/env bash
# assemble.sh <in> <dist>: gathers the release files from the verified
# artifacts in <in>/<name>/ into <dist>, refusing any file that is not one of
# the release service targets for HOKUM_VERSION (docs/releasing.md) or that
# two artifacts both hold, and requiring all six.
. "$(dirname "$0")/lib.sh"

in=$(posix "${1:?Usage: assemble.sh <in> <dist>}")
dist=$(posix "${2:?Usage: assemble.sh <in> <dist>}")
v=${HOKUM_VERSION:?Set HOKUM_VERSION}
mkdir -p "$dist"
for file in "$in"/*/*; do
  name=$(basename "$file")
  [ "$name" = SHA256SUMS ] && continue
  case "$name" in
    "hokum-sound-$v-linux-x64" | "hokum-sound-$v-linux-x64.deb" | "hokum-sound-$v-linux-x64.rpm" | \
      "hokum-sound-$v-linux-arm64" | "hokum-sound-$v-linux-arm64.deb" | "hokum-sound-$v-linux-arm64.rpm") ;;
    *) die "Unexpected release file $name." ;;
  esac
  [ ! -e "$dist/$name" ] || die "Two artifacts hold $name."
  cp "$file" "$dist/$name"
done
for label in x64 arm64; do
  for suffix in "" .deb .rpm; do
    [ -f "$dist/hokum-sound-$v-linux-$label$suffix" ] || die "The release lacks hokum-sound-$v-linux-$label$suffix."
  done
done
(cd "$dist" && ls -l && sha256 -- *) | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
