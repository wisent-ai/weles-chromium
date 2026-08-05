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
  Darwin/arm64)  PLATFORM="darwin-arm64"; APP="Chromium.app"; BIN="$BUILD_OUT/Chromium.app/Contents/MacOS/Chromium"; ENTRYPOINT="Chromium.app/Contents/MacOS/Chromium" ;;
  Darwin/x86_64) PLATFORM="darwin-x64"; APP="Chromium.app"; BIN="$BUILD_OUT/Chromium.app/Contents/MacOS/Chromium"; ENTRYPOINT="Chromium.app/Contents/MacOS/Chromium" ;;
  Linux/x86_64)  PLATFORM="linux-x64"; APP="chromium"; BIN="$BUILD_OUT/chromium/chrome"; ENTRYPOINT="chromium/chrome" ;;
  *) echo "ERROR: unsupported platform $uname_s/$uname_m" >&2; exit 1 ;;
esac

if [[ ! -x "$BIN" ]]; then echo "ERROR: built binary not found at $BIN" >&2; exit 1; fi
if ! command -v gh >/dev/null 2>&1; then echo "ERROR: gh CLI is required" >&2; exit 1; fi
ACTOR="$(gh api user --jq .login)"
APPROVERS="$(gh variable get WELES_RELEASE_APPROVERS --repo "$REPO")"
if ! printf '%s\n' "$APPROVERS" | tr ',' '\n' \
  | sed 's/^[[:space:]]*//;s/[[:space:]]*$//' | grep -Fqx "$ACTOR"; then
  echo "ERROR: ${ACTOR:-current GitHub actor} is not an allowlisted Weles release operator" >&2
  exit 1
fi
if ! git -C "$REPO_ROOT" diff --quiet || ! git -C "$REPO_ROOT" diff --cached --quiet; then
  echo "ERROR: commit tracked Chromium release inputs before publishing" >&2
  exit 1
fi

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
tar -chzf "$TMP/$ASSET" -C "$BUILD_OUT" "$APP"
if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$TMP/$ASSET" > "$TMP/$ASSET.sha256"
else
  sha256sum "$TMP/$ASSET" > "$TMP/$ASSET.sha256"
fi
CAPABILITIES_SHA256="$(openssl dgst -sha256 -r "$REPO_ROOT/browser-capabilities.json" | awk '{print $1}')"
SOURCE_REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD)"
PATCH_TREE="$(git -C "$REPO_ROOT" rev-parse HEAD:patches)"
FINAL_TAG="$TAG"
CANDIDATE_TAG="candidate-$FINAL_TAG-${SOURCE_REVISION:0:8}"
jq -n \
  --arg schema "weles.browser-candidate.v1" \
  --arg engine "chromium" \
  --arg finalTag "$FINAL_TAG" \
  --arg candidateTag "$CANDIDATE_TAG" \
  --arg sourceRevision "$SOURCE_REVISION" \
  --arg patchTree "$PATCH_TREE" \
  --arg platform "$PLATFORM" \
  --arg entrypoint "$ENTRYPOINT" \
  --arg artifact "$ASSET" \
  --arg artifactSha256 "$(awk 'NF { print $1; exit }' "$TMP/$ASSET.sha256")" \
  --arg capabilitiesSha256 "$CAPABILITIES_SHA256" \
  '{schema: $schema, engine: $engine, finalTag: $finalTag, candidateTag: $candidateTag, sourceRevision: $sourceRevision, patchTree: $patchTree, platform: $platform, entrypoint: $entrypoint, artifact: $artifact, artifactSha256: $artifactSha256, capabilitiesSha256: $capabilitiesSha256, status: "candidate"}' \
  > "$TMP/release-metadata.json"
cp "$REPO_ROOT/browser-capabilities.json" "$TMP/browser-capabilities.json"

gh release create "$CANDIDATE_TAG" \
  "$TMP/$ASSET" "$TMP/$ASSET.sha256" \
  "$TMP/browser-capabilities.json" "$TMP/release-metadata.json" \
  --repo "$REPO" --target "$SOURCE_REVISION" --prerelease \
  --title "$CANDIDATE_TAG" \
  --notes "Candidate bytes for $FINAL_TAG. Production promotion must reuse these exact bytes and attach Probierz evidence bound to the artifact SHA-256."
printf '%s\n' "$CANDIDATE_TAG"
