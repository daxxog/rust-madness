# rust-madness — the browser play page (public/site)

This directory is the GitHub Pages root: the plain W1 probe page is now the
real play page, styled after the original's archived site. `glue.js` is the
unchanged W1/W2 host (canvas, probe hook, audio bridge, resume-on-gesture);
only the chrome around it changed in W3.

## Files

- `index.html` — the archive-era chrome + the 550×400 `<canvas id="screen">`.
  The three notice documents are **rendered into the footer** between the
  `<!-- BEGIN GENERATED NOTICES -->` / `<!-- END GENERATED NOTICES -->` markers;
  do not hand-edit that block (see "Footer mechanism" below).
- `glue.js` — raw-wasm loader, rAF loop, RGB→RGBA blit, DOM-code → Java-`Event`
  code key table (mirrors `window.rs`'s WATCH), and the `AudioBus` bridge: a
  suspended-at-boot `AudioContext` resumed on the first gesture, a main-thread
  `ScriptProcessorNode` that pulls the mono mix at the context's native rate,
  and the i16→f32 / mono→channels conversion. `window.__nfm.audio` exposes the
  probe counters. It also supplies the `env` imports `js_ls_get`/`js_ls_set`
  that back the browser cookie jar (see "Persistence bridge" below), retires
  the `#loading` line on the first blit (or shows the error on a failed boot)
  and wires the `#fullscreen` toggle.
- `render-notices.py` — the tiny markdown→HTML build step that regenerates the
  footer block (see below).
- `hello.gif`, `sign.gif`, `loadbar.gif` — **vendored copies of the game's own
  art** (`assets/graphics/{hello,sign,loadbar}.gif`, byte-identical). They are
  the game's imagery, not new artwork: `hello.gif` (550×217) is the opening
  warning screen used as the intro banner, `sign.gif` (75×66) is the "!" icon
  used as the title accent and favicon, `loadbar.gif` (238×21) is the loading
  bar beside the applet. They are already covered by the site's original-game
  content notice in `../NOTICES.md`; note their provenance there if editing it.
- `rust-madness.wasm` — **derived, NOT committed.** Cargo builds
  `nfm_web.wasm`; the build recipe and the release pipeline rename it to
  `rust-madness.wasm` for the site (that is the name `glue.js` fetches). Built by:

  ```sh
  tools/pack.sh                      # embed assets into resources.pack
  cargo build --release --target wasm32-unknown-unknown -p nfm-web
  cp target/wasm32-unknown-unknown/release/nfm_web.wasm public/site/rust-madness.wasm
  ```

  The release build is required: `nfm-core`'s `pack.rs` serves every asset
  from the embedded `resources.pack` in release mode, whereas a debug wasm
  would try filesystem reads (which fail on wasm32-unknown-unknown) and the
  boot would fail. The `v*` tag workflow exported to `.github/workflows/release.yml`
   performs this copy and deploy-time rename; see "Release pipeline" below.

## Persistence bridge (localStorage cookie jar)

The original applet kept its progress in the **browser's cookie jar** through
`JSObject` (`GameSparker.java`'s `SetCookie`/`GetCookie` seam, T1). The desktop
port's `nfm-cookies.txt` is the documented stand-in for that jar; on wasm
there is no filesystem, so the web port uses **`localStorage`** as the
stand-in instead — the same per-origin, page-managed storage the original
actually used.

- One entry, key `nfm-cookies`, holds the entire `key=value\n` jar text (the
  same bytes the desktop `save()` writes to its file). `unlocked`, `usercar`
  and `gameprfact` survive a page refresh; a fresh origin starts at −1 exactly
  like a missing file.
- `nfm-core`'s `CookieStore::load`/`save` branch on `target_arch = "wasm32"`.
  The wasm branch reads the jar through `js_ls_get(buf, cap) -> usize` into a
  bounded 16 KiB scratch (a truncated read drops its last partial line) and
  writes it through `js_ls_set(ptr, len)`. Both are `env` imports supplied by
  `glue.js`; the host path-resolution/flush code is untouched and not compiled
  into the wasm module.
- `glue.js` owns all failure handling: `getItem`/`setItem` are wrapped in
  `try/catch`, so private mode, a disabled API or a full quota degrade to a
  session-only jar (reads return 0 / writes no-op) instead of trapping wasm or
  logging an uncaught error.

## Applet chrome: loading line and fullscreen

- The archive-era `applet loading...` line (`#loading`, beside `loadbar.gif`)
  is removed on the **first canvas blit** — the honest "painted" signal. If
  boot rejects, its text is replaced with the error instead, so a failed load
  never masquerades as a loading one.
- The small `[ fullscreen ]` text button (`#fullscreen`) in the controls line
  toggles `canvas.requestFullscreen()` / `document.exitFullscreen()`; Esc exits
  natively. Sizing is CSS-only under `#screen:fullscreen`: the canvas scales to
  `min(100vw, calc(100vh * 550 / 400))` with `aspect-ratio: 550 / 400`,
  `object-fit: contain` and `image-rendering: pixelated`, giving crisp pixels
  and black letterboxing with no JS resize logic.

## Where the look came from

- `reference/run/mad.html` — the 2007-10-09 capture of
  `radicalplay.com/madness/game.html`: `<body bgcolor="#000000">` with a
  `<center><applet code="GameSparker.class" width="550" height="400">`.
  Gives the black page, the centred window and the exact 550×400 canvas.
- One Wayback CDX query for `radicalplay.com/madness/*` (text/html, 200, one
  request) plus one raw `/id_/` fetch of the earliest `madness/` index
  (2005-05-13), also one request. Gives the surrounding chrome: body colours
  `#080029` / `#b9cbff`, Verdana/Arial small fonts, the `#e8ecff` nav table
  with `#30309C` links, the centred title treatment and the start-game button.
  The page here uses the game page's black background with that index's link
  colours, font stack and nav-strip motif; the yellow-orange title colour is a
  restrained stand-in because the original's `web/madness.gif` title image was
  not recovered into `assets/`. No further archive requests were made.

## Footer mechanism (no drift)

The footer is **inline pre-rendered HTML**, so the credits and all three
notices are visible the moment the page loads — nothing is fetched or clicked.
The canonical sources are the markdown/text files one level up, in `public/`:

```
public/NOTICES.md   public/PROVENANCE.md   public/LICENSE
```

Regenerate the block after editing any of them:

```sh
python3 public/site/render-notices.py
```

The script reads those three files, converts headings / bullets / bold / code /
links to HTML with a small in-script converter, and replaces everything
between the two markers in `index.html`. Because the HTML is generated from
the canonical sources rather than retyped, the rendered footer cannot drift
from them; the `BEGIN`/`END` comment markers make the generated region obvious
and keep regeneration idempotent. Run the script from any working directory —
it resolves paths relative to itself.

## Build recipe (full page)

```sh
# 1. rebuild the wasm (derived artifact, never committed); cargo emits nfm_web.wasm,
#    renamed to rust-madness.wasm for the site
tools/pack.sh
cargo build --release --target wasm32-unknown-unknown -p nfm-web
cp target/wasm32-unknown-unknown/release/nfm_web.wasm public/site/rust-madness.wasm

# 2. regenerate the rendered notices footer from the canonical markdown
python3 public/site/render-notices.py

# 3. serve (artifacts under target/, never committed)
mkdir -p target/probe-site
cp public/site/index.html public/site/glue.js \
   public/site/hello.gif public/site/sign.gif public/site/loadbar.gif \
   target/probe-site/
cp target/wasm32-unknown-unknown/release/nfm_web.wasm target/probe-site/rust-madness.wasm
cd target/probe-site && python3 -m http.server <port>
```

Then open `http://127.0.0.1:<port>/`. Every asset the page references
(`glue.js`, `rust-madness.wasm`, `hello.gif`, `sign.gif`, `loadbar.gif`) lives in
this directory. The downloads section's links are absolute GitHub Releases
URLs built from the single `NFM_REPO_SLUG` constant at the top of `index.html`,
wired to the real `owner/repo` slug `daxxog/rust-madness` — the only place
that slug appears on the page.

## Release pipeline (`v*` tags)

The tag workflow `public/.github/workflows/release.yml` (exported to
`.github/workflows/release.yml` by `tools/public-export.sh`) runs four jobs:

1. **gates** — re-runs the whole CI fidelity battery on the tagged commit:
   re-derive `reference/` from `mad.jar`, the Java behavior gate (4792/0 on
   both the local and Temurin-8 container legs), the pixel gate (172/0, the
   container's shipped font as the font authority), the Rust mirrors
   (207 + 42 workspace tests; 4808/0 logic + 172/0 pixel), and the hermetic
   `PROVENANCE.md` digest check. Nothing is built until all four pass.
2. **build** — runs `tools/pack.sh` in every leg, then builds four artifacts
   on four runners: macOS `aarch64-apple-darwin` (Apple-silicon runner) and
   `x86_64-apple-darwin` (Intel runner), Linux `x86_64-unknown-linux-gnu`
   built natively on ubuntu after `sudo apt-get install -y libasound2-dev
   libudev-dev` (no cross toolchain, no private config), and the
   `wasm32-unknown-unknown` `nfm-web` module. The cargo targets stay
   `nfm-render`/`nfm-web`; packaging renames the shipped files. Each is tarred
   with a README: `rust-madness-mac-aarch64.tar.gz`,
   `rust-madness-mac-x86_64.tar.gz`, `rust-madness-linux-x86_64.tar.gz`
   (each containing the `rust-madness` executable) and `rust-madness-wasm.tar.gz`
   (containing `rust-madness.wasm`).
3. **publish** — attaches the four tarballs to a GitHub Release named for the
   tag, with the annotated tag message as the release body.
4. **pages** — deploys **only** `public/site/` plus the built
   `rust-madness.wasm` (the `nfm_web.wasm` output renamed to the literal name
   `glue.js` fetches) to GitHub Pages via `actions/configure-pages@v5` +
   `actions/upload-pages-artifact@v3` + `actions/deploy-pages@v4`. No tarballs,
   notes or private sources ship in the site.

The self-contained Mac and Deck binaries need no data files at runtime; the
score file `nfm-cookies.txt` is created beside the executable (cwd first, then
the executable's directory). The Pages wasm is derived and is never committed.
