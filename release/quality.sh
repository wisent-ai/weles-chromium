#!/usr/bin/env bash
# The fmt gate of weles-chromium: every file under patches/ is a well-formed
# git patch, read the way `git am` reads it. The series applies to an upstream
# tree no release carries, so applying it is the build's job
# (release/build.sh); this gate refuses a malformed or empty series before a
# builder spends hours on it.
set -euo pipefail
cd "$(dirname "$0")/.."
shopt -s nullglob
series=(patches/*.patch)
if [ -z "${series[*]}" ]; then
  echo "release/quality.sh: patches/ holds no .patch file" >/dev/stderr
  false
fi
git apply --numstat "${series[@]}" >/dev/null
echo "release/quality.sh: ${#series[@]} patches parse"
