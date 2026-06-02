# weles-chromium

The weles fingerprint-defense patch series for Chromium, as a reviewable
patch set (parallel to [`wisent-ai/weles-firefox`](https://github.com/wisent-ai/weles-firefox)).

The compiled binaries ship as GitHub Releases on
[`wisent-ai/weles`](https://github.com/wisent-ai/weles)
(tag scheme `chromium-<upstream-version>-weles.N`) and hosts install them via
`weles/scripts/chromium/download.sh`. **This repo is the _source of truth for
the C++ delta_** — the small set of engine-level changes that turn a stock
Chromium into the weles binary. It does **not** vendor the ~116 GB Chromium
checkout; it carries only the patches that apply on top of a pinned upstream.

## Upstream base

| | |
|---|---|
| Upstream version | **147.0.7727.108** |
| Fork point (patch base) | `e74a8f5bfafeb` (last upstream commit before the weles series) |
| Local working branch | `weles-147` in `chromium-build/src` |

Bump the upstream version + fork point together on a rebase, and re-export the
series (see "Re-exporting" below).

## What the patches do

Every override is **opt-in at launch via `--weles-fingerprint=<json>`**. With
the switch unset, `WelesFingerprintConfig::Get()` returns `nullptr` and every
patched call site defers to stock Chromium — so an unflagged binary is a
drop-in upstream chrome. The JSON schema mirrors `weles.fingerprint.toCppConfig`
(TypeScript) → the C++ struct in `weles_fingerprint_config.h`.

| Patch | Surface |
|---|---|
| `0001-Weles-modifications-rebased-onto-Chromium-147...` | The bulk of the delta: new `third_party/blink/renderer/platform/weles_fingerprint_config.{cc,h}` singleton + `--weles-fingerprint` switch (`third_party/blink/common/switches.*`), and the call-site overrides — `navigator.*` (UA, platform, vendor, languages, hardwareConcurrency, deviceMemory), `screen.*`, WebGL `UNMASKED_VENDOR/RENDERER`, canvas noise removal (`image_data_buffer.cc`), WebAudio noise, WebRTC ICE IP override (`rtc_ice_candidate_platform.cc`), client hints `sec-ch-ua-*` (`user_agent_utils.cc`), locale, plus proxy-socket / network hardening and `chrome/renderer/webstore_extension_bindings.*`. |
| `0002-input_handler-backdate-CDP-event-timestamps-...` | Backdates CDP-routed input event timestamps and stamps realistic pointer pressure / `movementX/Y` provenance so dispatched input is indistinguishable from OS-queued input. |
| `0003-weles-remove-unreachable-code-in-PrepareForAuthResta...` | Cleanup in the proxy auth-restart path. |
| `0004-weles-wrap-canvas-noise-pixmap-access-in-UNSAFE_BUFF...` | Wraps the canvas-noise pixmap access in `UNSAFE_BUFFERS()` for the newer Chromium buffer-safety lint. |
| `0005-weles-drop-unused-cfg-binding-in-Navigator-webdriver...` | Trivial unused-binding cleanup in `Navigator::webdriver()`. |
| `UNCOMMITTED-working-tree.patch` | **Not part of the reviewed series.** A snapshot of 5 files modified in the local `weles-147` working tree but not yet committed (`input_handler.{cc,h}`, `v8_binding_for_core.cc`, `thread_restrictions.cc`, `tab_stats_data_store.cc`; +40/−9). Captured so the work isn't lost on a fresh checkout — commit it into `weles-147` and re-export, or drop it. |

## Applying

```bash
# Onto a fresh upstream checkout pinned at the fork point:
bash apply.sh /path/to/chromium/src
# or by hand, from chromium/src:
git checkout -b weles-147 e74a8f5bfafeb
git am /path/to/weles-chromium/patches/0001-*.patch \
       /path/to/weles-chromium/patches/0002-*.patch \
       /path/to/weles-chromium/patches/0003-*.patch \
       /path/to/weles-chromium/patches/0004-*.patch \
       /path/to/weles-chromium/patches/0005-*.patch
```

If context has shifted on a newer upstream, `git am -3` (3-way) resolves most
hunks; fix any `.rej`, `git add`, `git am --continue`.

## Building & releasing

Building is driven from the `weles` repo, not here — it expects the patched tree
at `../chromium-build/src` with an `out/Weles` GN config:

```bash
# in weles/
bash scripts/chromium/build.sh             # autoninja (~4 h), then auto-publish
bash scripts/chromium/build.sh --dry-run   # build, preview the release
```

`release.sh` reads the version from the built binary, cuts a fresh
`chromium-<ver>-weles.N` release on `wisent-ai/weles`, bumps the pin in
`download.sh`, and pushes so every host auto-deploys within ~60 s.

## Re-exporting the series after new work

```bash
# in chromium-build/src, after committing onto weles-147:
git format-patch <fork-point>..weles-147 -o /path/to/weles-chromium/patches --no-signature
```

Then update the upstream version + fork point in this README and push.
