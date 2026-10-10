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

bash "$WISENT_SOURCE_DIR/apply.sh" "$WELES_CHROMIUM_SRC"
(cd "$WELES_CHROMIUM_SRC" && autoninja -C out/Weles chrome)

app="$WELES_CHROMIUM_SRC/out/Weles/Chromium.app"
if [ ! -d "$app" ]; then
  echo "release/build.sh: the build ended without $app" >/dev/stderr
  false
fi
mkdir -p "$WISENT_OUTPUT_DIR/stage"
ditto "$app" "$WISENT_OUTPUT_DIR/stage/Chromium.app"
