#!/bin/sh
# Wraps a Linux release executable in the hokum-sound .deb and .rpm
# (packaging/linux/nfpm.yaml):
#
#   package-linux.sh <executable> <version> <x64|arm64> <dist>
#
# writes <dist>/hokum-sound-<version>-linux-<arch>.deb and .rpm, which the Release
# workflow publishes as the linux-<arch>-deb and linux-<arch>-rpm targets.
# Requires nfpm (NFPM names another binary).
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
[ $# -eq 4 ] || {
  echo "Usage: $0 <executable> <version> <x64|arm64> <dist>" >&2
  exit 1
}
executable=$(realpath "$1") version=$2 label=$3 dist=$(realpath "$4")
nfpm=${NFPM:-nfpm}
command -v "$nfpm" >/dev/null 2>&1 || {
  echo "Packaging requires nfpm." >&2
  exit 1
}
case "$label" in
  x64) arch=amd64 ;;
  arm64) arch=arm64 ;;
  *)
    echo "The architecture must be x64 or arm64, not $label." >&2
    exit 1
    ;;
esac
test "$(head -c 4 "$executable" | od -An -tx1 | tr -d ' ')" = 7f454c46

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT HUP INT TERM
install -m755 "$executable" "$temporary/sound"
cp "$root/packaging/hokum-sound.desktop" "$root/LICENSE" "$temporary/"

# Debian and RPM order a tilde before a release, and neither allows a hyphen
# in the upstream version: 0.0.0-dev-42 is 0.0.0~dev.42, 0.2.0-beta.1 is
# 0.2.0~beta.1.
package_version=$(printf '%s' "$version" | sed -e 's/-/~/' -e 's/-/./g')

for format in deb rpm; do
  (
    cd "$temporary"
    HOKUM_PACKAGE_ARCH=$arch HOKUM_PACKAGE_VERSION=$package_version \
      "$nfpm" package --config "$root/packaging/linux/nfpm.yaml" --packager "$format" \
      --target "$dist/hokum-sound-$version-linux-$label.$format" >&2
  )
  echo "$dist/hokum-sound-$version-linux-$label.$format"
done
