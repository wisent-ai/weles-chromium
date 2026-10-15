#!/usr/bin/env bash
# The build step Stado's release worker runs for weles-chromium
# (.wisent-release.json). It applies this checkout's patch series onto the
# upstream fork point in the builder's own Chromium checkout, builds the
# out/Weles target and stages the native browser bundle for the release archive.
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
: "${WISENT_PLATFORM:?WISENT_PLATFORM is required: the release worker names the target platform}"

refuse() { printf 'Chromium release build: %s\n' "$*" >/dev/stderr; false; }
for tool in git jq autoninja; do
  command -v "$tool" >/dev/null || refuse "patch application and compilation require $tool"
done
host="$(uname -s)-$(uname -m)"
case "$WISENT_PLATFORM:$host" in
  darwin-arm64:Darwin-arm64)
    target=chrome
    command -v ditto >/dev/null || refuse "darwin-arm64 packaging requires ditto"
    ;;
  linux-amd64:Linux-x86_64)
    target=chrome/installer/linux:stable_deb
    command -v dpkg-deb >/dev/null || refuse "linux-amd64 packaging requires dpkg-deb"
    command -v gn >/dev/null || refuse "linux-amd64 packaging requires gn"
    extracted="$WISENT_OUTPUT_DIR/linux-package"
    [ ! -e "$extracted" ] && [ ! -L "$extracted" ] ||
      refuse "package extraction directory already exists: $extracted; source was not changed"
    ;;
  *) refuse "target $WISENT_PLATFORM cannot be built on $host" ;;
esac
[ -f "$WELES_CHROMIUM_SRC/out/Weles/build.ninja" ] ||
  refuse "the declared source has no configured build graph: $WELES_CHROMIUM_SRC/out/Weles/build.ninja; source was not changed"
[ ! -e "$WISENT_OUTPUT_DIR/stage" ] && [ ! -L "$WISENT_OUTPUT_DIR/stage" ] ||
  refuse "staging destination already exists: $WISENT_OUTPUT_DIR/stage; source was not changed"

bash "$WISENT_SOURCE_DIR/apply.sh" "$WELES_CHROMIUM_SRC"
(cd "$WELES_CHROMIUM_SRC" && autoninja -C out/Weles "$target")

mkdir -p "$WISENT_OUTPUT_DIR/stage"
case "$WISENT_PLATFORM" in
  darwin-arm64)
    app="$WELES_CHROMIUM_SRC/out/Weles/Chromium.app"
    [ -d "$app" ] || refuse "the build ended without $app"
    ditto "$app" "$WISENT_OUTPUT_DIR/stage/Chromium.app"
    ;;
  linux-amd64)
    package="$(cd "$WELES_CHROMIUM_SRC" && gn desc out/Weles //chrome/installer/linux:stable_deb outputs)"
    case "$package" in
      //out/Weles/*.deb) package="$WELES_CHROMIUM_SRC/${package#//}" ;;
      *) refuse "stable_deb returned an unexpected package output: $package" ;;
    esac
    [ -f "$package" ] || refuse "the build ended without $package"
    dpkg-deb --extract "$package" "$extracted"
    # Upstream owns the package layout as well as its complete resource set.
    source "$WELES_CHROMIUM_SRC/chrome/installer/linux/common/chromium-browser.info"
    : "${INSTALLDIR:?upstream chromium-browser.info must declare INSTALLDIR}"
    bundle="$extracted$INSTALLDIR"
    [ -x "$bundle/chrome" ] || refuse "upstream package lacks executable $bundle/chrome"
    [ ! -e "$WISENT_OUTPUT_DIR/stage/chromium" ] || refuse "Chromium staging destination already exists"
    mv "$bundle" "$WISENT_OUTPUT_DIR/stage/chromium"
    ;;
esac
