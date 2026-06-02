#!/usr/bin/env bash
# Apply the weles Chromium patch series onto an upstream chromium/src checkout.
#
# Usage:
#   bash apply.sh /path/to/chromium/src
#
# The patches are git-am mailbox patches (they carry author + message), so they
# replay as real commits on a fresh `weles-147` branch.
set -euo pipefail

SRC="${1:-}"
FORK_POINT="e74a8f5bfafeb"          # last upstream commit before the weles series
BRANCH="weles-147"
HERE="$(cd "$(dirname "$0")" && pwd)"

if [[ -z "$SRC" || ! -d "$SRC/.git" ]]; then
  echo "usage: bash apply.sh /path/to/chromium/src   (a git checkout)" >&2
  exit 1
fi

cd "$SRC"
echo "[apply] checking fork point $FORK_POINT is present ..." >&2
if ! git cat-file -e "${FORK_POINT}^{commit}" 2>/dev/null; then
  echo "ERROR: $FORK_POINT not in this checkout — fetch upstream at 147.0.7727.108 first" >&2
  exit 1
fi

echo "[apply] creating branch $BRANCH at $FORK_POINT ..." >&2
git checkout -B "$BRANCH" "$FORK_POINT"

echo "[apply] git am the numbered series ..." >&2
if ! git am "$HERE"/patches/0[0-9]*-*.patch; then
  echo "[apply] direct am failed — retrying with 3-way merge" >&2
  git am --abort || true
  git checkout -B "$BRANCH" "$FORK_POINT"
  git am -3 "$HERE"/patches/0[0-9]*-*.patch
fi

echo "[apply] done. Series applied on $BRANCH." >&2
