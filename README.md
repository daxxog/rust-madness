# rust-madness — a Rust port of the 2006 Java applet Need for Madness

A byte-faithful Rust port of **Need for Madness**, the 2006 Java browser
applet. It rebuilds the original's engine, renderer and tracker-music player
from the distributed `mad.jar` and the game's server-side asset bundle, then
proves equivalence by replaying the original's own behavior and pixel suites.

## Credits and origin

The game credits itself, on its in-game credits screen, to **Omar Waly** at
**Radicalplay.com**. The recovered strings, verbatim, are `"At Radicalplay.com"`,
`"By Omar Waly"`, and `"For any comments: Omar@radicalplay.com"`
(`xtGraphics.java`, credits screen, lines 1483–1493); that screen also names
the game-testing team and `"Music From www.ModArchive.com"`. The bundled
music engine is the `RadicalMod` / `ModSlayer` tracker player. The jar's
`ModSlayer.COPYRIGHT` constant (`ModSlayer.java:5`) is the **empty string** —
there is no embedded copyright notice in the bytecode to quote. The applet
was distributed from `http://www.radicalplay.com/madness/mad.jar`; the
Wayback Machine holds three 200-OK captures (2006-05-05, 2006-10-25,
2007-07-02) whose CDX digests all match the shipped jar byte-for-byte (see
`PROVENANCE.md`). The 2006 date is that earliest capture record — no date
is encoded in the jar itself.

## Why

The original is Java 1.1-era bytecode (class file 45.3) containing a custom
software 3D engine and a ProTracker-style music player. Its host page and
server-side resources are no longer served. This port keeps a runnable,
self-contained version alive and leaves a machine-checkable record of what
the original did, so the game can still be studied and played after the
original distribution has disappeared.

## Proof of fidelity

Two suites are run against the **original**, compiled from decompiled sources
unchanged:

| gate | result |
|---|---|
| Java behavior suite (`Suite.java`), local JDK 8 | 4792 pins, 0 failures |
| Java behavior suite, Temurin 8 container | 4792 pins, 0 failures |
| Java pixel suite (`PixelSuite.java`), 84 frame rows | 172 pins, 0 failures |

The Rust port replays the same pins from the same fixtures:

| mirror | result |
|---|---|
| logic mirror (`mirror_suite`) | 4808 pins, 0 failures — the 4792 Java pins plus harness-shape pins |
| pixel mirror (`mirror_pixel_suite`) | 172 pins, 0 failures — byte-exact frame md5s |

`cargo test --workspace` runs both mirrors; the original-side gates live in
`tools/java-tests.sh` and `tools/pixel-tests.sh`.

## Build and run

Stable Rust (edition 2024); use the rustup toolchain
(`PATH=~/.cargo/bin:$PATH`). Run from the repository root so asset paths
resolve as `assets/...`.

```sh
cargo run -p nfm-render                                   # play — stage 1, 550x400 window + audio
cargo run -p nfm-render -- play [stage]                   # play a chosen stage
cargo run -p nfm-render -- stage <n> [out.bmp] [frames]   # headless: stage-load flyover
cargo run -p nfm-render -- menu <n> [frames] [out]        # headless: full applet menu walk
cargo run -p nfm-render -- race <stage> [out] [frames] [starcnt] [view] [press] [replays] [crash]
cargo run -p nfm-render -- gif <zipo> <entry> <out.bmp>   # headless: decode one GIF entry
cargo run -p nfm-render -- <zipo> <model> <out.bmp> [yaw] # headless: one car model, BMP
tools/pack.sh                                             # derive resources.pack
```

A release build is self-contained: `tools/pack.sh` packs every play-time
resource into `resources.pack`, `crates/nfm-core/build.rs` embeds it, and the
resulting binary reads no resource file from the filesystem. Debug builds
keep the filesystem-first development flow. Scores persist to
`nfm-cookies.txt` (the stand-in for the browser cookie jar).

## Steam Deck controls

Locked mapping from `crates/nfm-render/src/gamepad.rs`. gilrs names are
**positional** while the Deck's printed labels are **Xbox-style**: A = `South`
(bottom), B = `East` (right), X = `West` (left), Y = `North` (top). Read the
table by physical label.

| pad                          | game concept                  | code |
|------------------------------|-------------------------------|------|
| RT (`RightZ` / `RightTrigger2`) | accelerate                 | 1004 |
| X (left, `West`)             | brake / reverse               | 1005 |
| LT (`LeftZ` / `LeftTrigger2`) | handbrake                    | 32   |
| left stick X + D-pad L/R     | steer (binary, hysteresis)    | 1006 / 1007 |
| A (bottom, `South`)          | confirm / enter (every screen)| 10   |
| Y (top, `North`)             | arrow-switch (`arrace`) toggle| 97   |
| B (right, `East`)            | unmapped (free)               | —    |
| Start (`Menu`/`Start`)       | always enter (pause in race)  | 10   |
| left stick Y                 | in race: unmapped             | —    |
|                              | elsewhere: menu up / down     | 1004 / 1005 |
| D-pad up                     | in race: camera view cycle    | 118  |
|                              | elsewhere: menu up            | 1004 |
| D-pad down                   | in race: unmapped             | —    |
|                              | elsewhere: menu down          | 1005 |
| LB (`LeftTrigger`) / L4 / L5 | music mute                    | 109  |
| RB (`RightTrigger`) / R4 / R5| sound-effects mute            | 110  |

## Preservation stance

Original game content — code, art, music and data — is © its original
authors (Omar Waly / Radicalplay.com). It is included here non-commercially
for preservation and study; no ownership of that content is claimed. The
**MIT license** (`LICENSE`) covers this project's own code and documentation
only. See `NOTICES.md` for third-party notices and `PROVENANCE.md` for
artifact origins and derived files.

## Play in the browser

Coming soon: a WebAssembly host is planned. There is no browser build yet.
