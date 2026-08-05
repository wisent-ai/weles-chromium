# weles-chromium

The weles fingerprint-defense patch series for Chromium, as a reviewable
patch set (parallel to [`wisent-ai/weles-firefox`](https://github.com/wisent-ai/weles-firefox)).

Compiled binaries ship from this repository's own GitHub Releases channel using
the tag scheme `chromium-<upstream-version>-weles.N`. Weles hosts consume that
channel through `weles/scripts/chromium/download.sh`.

This repository is the source of truth for the C++ delta and the release
contract. It does not vendor the Chromium checkout; it carries only the patches
that apply on top of a pinned upstream.

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
| `0006-weles-stamp-movement_x-y-on-CDP-synthesized-mouse-ev...` | Stamps `movement_x/movement_y` on CDP-synthesized mouse events from the InputHandler's tracked previous position (sentinel so the first event reports movement 0, like a real mouse entering the window). CDP leaves these at 0 by default — the bot signature LinkedIn `/apfc/collect` and Arkose read. |
| `0007-weles-mirror-release-channel-behavior-for-3-dcheck_a...` | Neutralizes 3 `dcheck_always_on` aborts that fire during real automation where stock release Chromium silently continues: `AssertBlockingAllowed` (TikTok captcha WASM sync I/O), `TabStatsDataStore::OnWindowRemoved` (TikTok verify SDK popup-iframe counter underflow), `ToV8ContextMaybeEmpty` (Arkose/GitHub nav transient detached-frame context). Each mirrors release-channel behavior, not a blind bypass. |

## Applying

```bash
# Onto a fresh upstream checkout pinned at the fork point:
bash apply.sh /path/to/chromium/src
# or by hand, from chromium/src:
git checkout -b weles-147 e74a8f5bfafeb
git am /path/to/weles-chromium/patches/0*.patch
```

If context has shifted on a newer upstream, `git am -3` (3-way) resolves most
hunks; fix any `.rej`, `git add`, `git am --continue`.

## Building and releasing

Builds use an external patched Chromium checkout with an `out/Weles` GN config.
The release script reads the version from the built browser and publishes the
platform archive plus checksum to `wisent-ai/weles-chromium`:

```bash
bash scripts/build.sh

# Publish an existing build without rebuilding:
CHROMIUM_BUILD_OUT=/path/to/chromium/src/out/Weles bash scripts/release.sh
```

The authenticated `gh` actor must appear in the repository's comma-separated
`WELES_RELEASE_APPROVERS` variable; publication fails before packaging
otherwise.

The publisher creates a prerelease candidate named
`candidate-chromium-<upstream-version>-weles.N-<revision>`. The release workflow
verifies its declared digest and source revision, then emits GitHub artifact
provenance for those exact bytes. Production promotion reuses the candidate
archive only after the Weles evidence gate approves its digest; this repository
never commits into a consumer repository.

## Re-exporting the series after new work

```bash
# in chromium-build/src, after committing onto weles-147:
git format-patch <fork-point>..weles-147 -o /path/to/weles-chromium/patches --no-signature
```

Then update the upstream version + fork point in this README and push.
