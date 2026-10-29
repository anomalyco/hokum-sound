#!/usr/bin/env bash
# push-aur.sh <dir> | check: pushes the staged hokum-sound-bin package (PKGBUILD,
# .SRCINFO, and its files, staged and tested by scripts/release-aur.sh in a
# job without secrets) to the AUR with AUR_SSH_PRIVATE_KEY, or with `check`
# confirms the AUR accepts the key.
. "$(dirname "$0")/lib.sh"

package=hokum-sound-bin
files=(PKGBUILD .SRCINFO hokum-sound.desktop LICENSE "$package.install")
: "${AUR_SSH_PRIVATE_KEY:?Set AUR_SSH_PRIVATE_KEY to the AUR SSH private key}"
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
(umask 077 && printf '%s\n' "$AUR_SSH_PRIVATE_KEY" > "$temporary/key")
# aur.archlinux.org's published ED25519 host key.
echo 'aur.archlinux.org ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEuBKrPzbawxA/k2g6NcyV5jmqwJ2s+zpgZGZ7tpLIcN' > "$temporary/known_hosts"
export GIT_SSH_COMMAND="ssh -i $temporary/key -o IdentitiesOnly=yes -o UserKnownHostsFile=$temporary/known_hosts -o StrictHostKeyChecking=yes"

if [ "${1:-}" = check ]; then
  # list-repos succeeds only for a registered key; it names the account's packages.
  $GIT_SSH_COMMAND aur@aur.archlinux.org list-repos
  exit 0
fi
dir=$(posix "${1:?Usage: push-aur.sh <dir> | check}")
for file in "${files[@]}"; do [ -f "$dir/$file" ] || die "The staged package lacks $file."; done
[ "$(find "$dir" -type f ! -name SHA256SUMS | wc -l | tr -d ' ')" = "${#files[@]}" ] || die "The staged package has unexpected files."
version=$(sed -n 's/^pkgver=//p' "$dir/PKGBUILD")
# Every source the package names must already serve the bytes it pins.
paste <(sed -n 's/^\tsource_[a-z0-9_]* = [^:]*::\(.*\)$/\1/p' "$dir/.SRCINFO") \
  <(sed -n 's/^\tsha256sums_[a-z0-9_]* = //p' "$dir/.SRCINFO") > "$temporary/sources"
[ "$(wc -l < "$temporary/sources")" -eq 2 ] || die "Expected two architecture sources in .SRCINFO."
while IFS=$'\t' read -r url sum; do
  [[ "$url" =~ ^https://anoma\.ly/release/hokum/files/[0-9a-f]{64}/sound$ ]] ||
    die "Unexpected source $url."
  [ "$(curl -fsSL --retry 3 "$url" | sha256 - | cut -c1-64)" = "$sum" ] || die "$url does not serve $sum."
done < "$temporary/sources"
# Cloning a package that does not exist yet gives an empty repository; the
# first push creates hokum-sound-bin under the key's account.
git clone -q "ssh://aur@aur.archlinux.org/$package.git" "$temporary/aur"
for file in "${files[@]}"; do cp "$dir/$file" "$temporary/aur/$file"; done
cd "$temporary/aur"
git add "${files[@]}"
if git diff --cached --quiet; then
  echo "The AUR already has $package $version." >&2
  exit 0
fi
git -c user.name='Hokum Sound release' -c user.email='hokum@anoma.ly' commit -qm "Update to $version"
git push -q origin HEAD:master
echo "https://aur.archlinux.org/packages/$package"
