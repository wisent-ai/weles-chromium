#!/usr/bin/env bash
# The build step Stado's release worker runs for weles-chromium
# (.wisent-release.json). It applies this checkout's patch series onto the
# upstream fork point in the builder's own Chromium checkout, builds the
# out/Weles target and stages Chromium.app for the release archive.
#
# Inputs, all refused by name when missing:
#   WISENT_SOURCE_DIR, WISENT_OUTPUT_DIR  the release worker's contract
#   WELES_CHROMIUM_SRC                    the builder's chromium/src checkout
#                                         (it is tens of gigabytes, so no
#                                         release carries it; the builder that
#                                         holds it declares where)
# The fork point is the one browser-capabilities.json states, so the patch
# base and the declaration a Weles host reads cannot disagree.
set -euo pipefail

: "${WISENT_SOURCE_DIR:?WISENT_SOURCE_DIR is required: the release worker names the checked-out source}"
: "${WISENT_OUTPUT_DIR:?WISENT_OUTPUT_DIR is required: the release worker names the staging directory}"
: "${WELES_CHROMIUM_SRC:?WELES_CHROMIUM_SRC is required: the builder declares its chromium/src checkout, which holds the upstream tree the patches apply to}"

capabilities="$WISENT_SOURCE_DIR/browser-capabilities.json"
fork_point=$(sed -n 's/.*"forkPoint": *"\([0-9a-f]*\)".*/\1/p' "$capabilities")
: "${fork_point:?browser-capabilities.json states no forkPoint}"
version=$(sed -n 's/.*"upstreamVersion": *"\([^"]*\)".*/\1/p' "$capabilities")
: "${version:?browser-capabilities.json states no upstreamVersion}"

if ! git -C "$WELES_CHROMIUM_SRC" cat-file -e "$fork_point^{commit}"; then
  echo "release/build.sh: $WELES_CHROMIUM_SRC holds no commit $fork_point (Chromium $version); fetch that upstream before building" >/dev/stderr
  false
fi
git -C "$WELES_CHROMIUM_SRC" checkout --force -B "weles-$version" "$fork_point"
git -C "$WELES_CHROMIUM_SRC" am --three-way "$WISENT_SOURCE_DIR"/patches/*.patch
(cd "$WELES_CHROMIUM_SRC" && autoninja -C out/Weles chrome)

app="$WELES_CHROMIUM_SRC/out/Weles/Chromium.app"
if [ ! -d "$app" ]; then
  echo "release/build.sh: the build ended without $app" >/dev/stderr
  false
fi
mkdir -p "$WISENT_OUTPUT_DIR/stage"
ditto "$app" "$WISENT_OUTPUT_DIR/stage/Chromium.app"
