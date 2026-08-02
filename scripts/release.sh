#!/usr/bin/env bash
# Publish one locally built, patched Chromium artifact to this product's own
# GitHub Releases channel. The build checkout remains external to this patch repo.
set -euo pipefail

REPO="wisent-ai/weles-chromium"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_OUT="${CHROMIUM_BUILD_OUT:-$REPO_ROOT/../chromium-build/src/out/Weles}"

uname_s=$(uname -s)
uname_m=$(uname -m)
case "$uname_s/$uname_m" in
  Darwin/arm64)  PLATFORM="macos-arm64"; APP="Chromium.app"; BIN="$BUILD_OUT/Chromium.app/Contents/MacOS/Chromium" ;;
  Darwin/x86_64) PLATFORM="macos-x86_64"; APP="Chromium.app"; BIN="$BUILD_OUT/Chromium.app/Contents/MacOS/Chromium" ;;
  Linux/x86_64)  PLATFORM="linux-x86_64"; APP="chromium"; BIN="$BUILD_OUT/chromium/chrome" ;;
  *) echo "ERROR: unsupported platform $uname_s/$uname_m" >&2; exit 1 ;;
esac

if [[ ! -x "$BIN" ]]; then echo "ERROR: built binary not found at $BIN" >&2; exit 1; fi
if ! command -v gh >/dev/null 2>&1; then echo "ERROR: gh CLI is required" >&2; exit 1; fi

FULLVER="$("$BIN" --version | awk '{print $NF}')"
if [[ -z "$FULLVER" ]]; then echo "ERROR: could not read Chromium version" >&2; exit 1; fi
PREFIX="chromium-$FULLVER-weles."
PREFIX_RE="$(printf '%s' "$PREFIX" | sed 's/\./\\./g')"
MAXN="$(gh release list --repo "$REPO" --json tagName -q '.[].tagName' 2>/dev/null \
  | sed -n "s#^${PREFIX_RE}\([0-9][0-9]*\)$#\1#p" | sort -n | tail -1)"
NEXTN=$(( ${MAXN:-0} + 1 ))
TAG="${PREFIX}${NEXTN}"
VERSION="${TAG#chromium-}"
ASSET="weles-chromium-${VERSION}-${PLATFORM}.tar.gz"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
tar -czf "$TMP/$ASSET" -C "$BUILD_OUT" "$APP"
if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$TMP/$ASSET" > "$TMP/$ASSET.sha256"
else
  sha256sum "$TMP/$ASSET" > "$TMP/$ASSET.sha256"
fi

gh release create "$TAG" "$TMP/$ASSET" "$TMP/$ASSET.sha256" \
  --repo "$REPO" --title "$TAG" \
  --notes "Weles-patched Chromium $FULLVER for $PLATFORM. Source patches are the tagged tree in this repository."
printf '%s\n' "$TAG"
