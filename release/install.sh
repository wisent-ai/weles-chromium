#!/usr/bin/env bash
# Stado's verified release delivery endpoint; never builds or launches a browser.
set -euo pipefail

: "${WISENT_RELEASE_ARCHIVE:?WISENT_RELEASE_ARCHIVE is required}"
: "${WISENT_RELEASE_URI:?WISENT_RELEASE_URI is required}"
: "${WISENT_RELEASE_SHA256:?WISENT_RELEASE_SHA256 is required}"
: "${WISENT_PRODUCT:?WISENT_PRODUCT is required}"
: "${WISENT_VERSION:?WISENT_VERSION is required}"
: "${WISENT_PLATFORM:?WISENT_PLATFORM is required}"
: "${HOME:?HOME is required}"

refuse() { printf 'Chromium release delivery: %s\n' "$*" >/dev/stderr; false; }
[ "$WISENT_PRODUCT" = weles-chromium ] || refuse "unexpected product: $WISENT_PRODUCT"
[ "$WISENT_PLATFORM" = darwin-arm64 ] || refuse "unsupported platform: $WISENT_PLATFORM"
[ "$(uname -s)-$(uname -m)" = Darwin-arm64 ] || refuse "delivery requires a Darwin arm64 host"
case "$WISENT_VERSION" in
  ''|.*|*[![:alnum:]._+-]*) refuse "unsafe release version: $WISENT_VERSION" ;;
esac
expected_uri="stado://releases/$WISENT_PRODUCT/$WISENT_VERSION/$WISENT_PLATFORM/release.tar.gz"
[ "$WISENT_RELEASE_URI" = "$expected_uri" ] || refuse "release URI $WISENT_RELEASE_URI differs from $expected_uri"
[ -f "$WISENT_RELEASE_ARCHIVE" ] && [ ! -L "$WISENT_RELEASE_ARCHIVE" ] || refuse "archive is not a regular non-symlink file: $WISENT_RELEASE_ARCHIVE"
observed="$(shasum -a 256 "$WISENT_RELEASE_ARCHIVE")"
observed="${observed%% *}"
[ "$observed" = "$WISENT_RELEASE_SHA256" ] || refuse "archive checksum mismatch: expected $WISENT_RELEASE_SHA256; observed $observed"

# The canonical installed layout is the one Weles launch admission reads.
root="$HOME/.local/share/weles-chromium"
if [ -n "${WELES_CHROMIUM_DIR+x}" ]; then
  [ "$WELES_CHROMIUM_DIR" = "$root" ] || refuse "managed delivery requires WELES_CHROMIUM_DIR=$root; observed $WELES_CHROMIUM_DIR"
fi
mkdir -p "$root"
root="$(cd "$root" && pwd -P)"
destination="$root/$WISENT_VERSION"
receipt="$(printf 'release_uri=%s\narchive_sha256=%s\nplatform=%s' "$expected_uri" "$observed" "$WISENT_PLATFORM")"
binary="Chromium.app/Contents/MacOS/Chromium"
verify() {
  local tree="$1"
  [ -f "$tree/$binary" ] && [ -x "$tree/$binary" ] || refuse "missing executable: $tree/$binary"
  [ -f "$tree/.weles-release" ] && [ ! -L "$tree/.weles-release" ] || refuse "missing regular receipt: $tree/.weles-release"
  cmp -s "$tree/.weles-release" <(printf '%s\n' "$receipt") || refuse "installed receipt conflicts with selected release: $tree/.weles-release"
}
lock="$root/.install-$WISENT_VERSION.lock"
mkdir "$lock" || refuse "another installation owns $lock; inspect that delivery before removing its lock"
# Install the trap only after this process owns the lock. Refusal above must
# never remove another delivery's reservation.
staging=''
cleanup() {
  if [ -n "$staging" ]; then rm -rf -- "$staging"; fi
  rmdir "$lock"
}
trap cleanup EXIT
if [ -e "$destination" ] || [ -L "$destination" ]; then
  [ -d "$destination" ] && [ ! -L "$destination" ] || refuse "installed destination is not a regular directory: $destination"
  verify "$destination"
  printf 'Verified installed Chromium release: %s\n' "$destination"
  exit
fi

# Isolated extraction never replaces an installed release. macOS libarchive
# refuses archive members that escape this fresh directory.
staging="$(mktemp -d "$root/.install-$WISENT_VERSION.XXXXXXXX")"
/usr/bin/tar -xzf "$WISENT_RELEASE_ARCHIVE" -C "$staging"
[ -f "$staging/$binary" ] && [ -x "$staging/$binary" ] || refuse "release archive lacks executable $binary"
[ ! -e "$staging/.weles-release" ] && [ ! -L "$staging/.weles-release" ] || refuse "release archive must not supply an installation receipt"
printf '%s\n' "$receipt" > "$staging/.weles-release"
verify "$staging"
[ ! -e "$destination" ] && [ ! -L "$destination" ] || refuse "destination appeared during delivery: $destination"
mv -n "$staging" "$destination"
[ ! -e "$staging" ] || refuse "destination changed during publication: $destination"
# Publication transferred ownership; cleanup must not remove installed bytes.
staging=''
verify "$destination"
printf 'Installed verified Chromium release: %s\n' "$destination"
