//! `nfm-core` — faithful Rust port of the Need for Madness applet build
//! (`mad.jar`, class file format 45.3, 2005/2007 engine shell).
//!
//! Porting strategy: behavior-first faithfulness. Engine details (integer
//! truncation casts, table layouts, mutation order) port 1:1 with comments
//! referencing the original source lines, so results can be checked against
//! `reference/` (decompiled Java) and `assets/` (byte-verified game data).
//! Idiomatic abstractions come later, only once parity is proven.
//!
//! Data layer: `zipo` + `pack` (asset containers and the embedded resource
//! pack) + `model` (the `.rad` parser).
//! Render layer: `medium` (camera/backdrop), `plane` (painter), `conto`
//! (model orchestration + camera modes), `wheels` (wheel geometry),
//! `trackers` (shared surface registry), `color` (Java AWT color math),
//! `canvas` (drawing surface + BMP output), `gif` (sprite decoding),
//! `xtgraphics` (HUD/menu sprite pixel transforms).

pub mod audiobus;
pub mod audioclip;
pub mod canvas;
pub mod checkpoints;
pub mod color;
pub mod conto;
pub mod control;
pub mod cookies;
pub mod gamesparker;
pub mod gif;
pub mod host;
pub mod jpeg;
pub mod madness;
pub mod medium;
pub mod model;
pub mod modslayer;
pub mod pack;
pub mod plane;
pub mod png;
pub mod radicalmod;
pub mod random;
pub mod record;
pub mod roundrect;
pub mod shell;
pub mod stage;
pub mod tracker;
pub mod trackers;
pub mod wheels;
pub mod xtfont;
pub mod xtgraphics;
pub mod zipo;

// The pins' MD5 (RFC 1321), shared by the mirror and the inline pixel
// batteries. Test-only: production never hashes.
#[cfg(test)]
pub(crate) mod hash;

// The Phase B mirror suite (notes/PHASE-B.md): the Java behavior suite's
// pins replayed 1:1 against this port, consuming the same
// tests/fixtures/toml/*.toml fixtures. Test-only; see mirror/mod.rs.
#[cfg(test)]
pub(crate) mod mirror;
