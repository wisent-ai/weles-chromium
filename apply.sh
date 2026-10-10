#!/usr/bin/env bash
# Apply the weles Chromium patch series onto an upstream chromium/src checkout.
#
# Usage:
#   bash apply.sh /path/to/chromium/src
#
# The patches are git-am mailbox patches (they carry author + message), so they
# replay as real commits on a detached checkout, preserving existing branches.
set -euo pipefail

SRC="${1:-}"
HERE="$(cd "$(dirname "$0")" && pwd)"

if [[ -z "$SRC" || ! -d "$SRC" ]]; then
  echo "usage: bash apply.sh /path/to/chromium/src   (a git checkout)" >&2
  exit 1
fi

cd "$SRC"
actual=$(git rev-parse --show-toplevel)
if [[ "$(cd "$actual" && pwd -P)" != "$(pwd -P)" ]]; then
  printf 'Chromium source must name the checkout root, not a directory inside %s; source was not changed.\n' "$actual" >&2
  false
fi
git_dir=$(git rev-parse --absolute-git-dir)
for operation in rebase-apply rebase-merge MERGE_HEAD CHERRY_PICK_HEAD REVERT_HEAD sequencer; do
  if [[ -e "$git_dir/$operation" ]]; then
    printf 'Chromium source has unfinished Git operation %s; source was not changed.\n' "$operation" >&2
    false
  fi
done
changes=$(git status --porcelain=v1 --untracked-files=all)
if [[ -n "$changes" ]]; then
  printf 'Chromium source has uncommitted work; source was not changed:\n%s\n' "$changes" >&2
  false
fi
FORK_POINT=$(jq -er '.forkPoint | select(type == "string" and . != "")' "$HERE/browser-capabilities.json")
echo "[apply] checking declared fork point $FORK_POINT ..." >&2
git cat-file -e "${FORK_POINT}^{commit}"
echo "[apply] checking out $FORK_POINT without moving an existing branch ..." >&2
git checkout --detach --no-overwrite-ignore "$FORK_POINT"
echo "[apply] applying the patch series with three-way merge ..." >&2
git am --three-way "$HERE"/patches/*.patch
echo "[apply] done. Series applied on detached HEAD." >&2
