#!/usr/bin/env bash
# Build the external Chromium checkout patched from this repository, then publish
# the result through this repository's release channel.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="${CHROMIUM_BUILD_SRC:-$REPO_ROOT/../chromium-build/src}"
TARGET="${CHROMIUM_NINJA_TARGET:-chrome}"
DEPOT="${DEPOT_TOOLS:-$REPO_ROOT/../chromium-build/depot_tools}"

if [[ ! -d "$SRC/out/Weles" ]]; then
  echo "ERROR: $SRC/out/Weles missing" >&2
  exit 1
fi
if [[ -d "$DEPOT" ]]; then export PATH="$DEPOT:$PATH"; fi
if ! command -v autoninja >/dev/null 2>&1; then
  echo "ERROR: autoninja not on PATH" >&2
  exit 1
fi

( cd "$SRC" && autoninja -C out/Weles "$TARGET" )
CHROMIUM_BUILD_OUT="$SRC/out/Weles" exec "$REPO_ROOT/scripts/release.sh"
