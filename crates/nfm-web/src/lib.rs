//! `nfm-web` — the browser host spike: a playable first frame in a canvas.
//!
//! The game itself lives entirely in `nfm-core`. This crate is only the
//! host glue the applet's `main` used to supply: boot the same spine as
//! `nfm-render::play_mode`, pump `Shell::frame` from JavaScript's rAF, and
//! hand the finished RGB canvas back to JS through a raw wasm ABI.
//!
//! The whole body is behind `cfg(target_arch = "wasm32")`, so host-side
//! workspace builds (and CI) compile this crate to an empty lib and stay
//! untouched — minifb/cpal/gilrs are never pulled into the wasm graph.
//!
//! ABI (wasm import module `env`, the `wasm32-unknown-unknown` default):
//!
//! | direction | symbol | signature |
//! |-----------|--------|-----------|
//! | import    | `js_now_ms` | `() -> f64` — wall clock in ms (never `Instant`/`SystemTime`, which panic on wasm) |
//! | import    | `js_log` | `(ptr: *const u8, len: usize) -> ()` — UTF-8 diagnostic |
//! | export    | `nfm_init` | `() -> ()` — one-time boot (idempotent) |
//! | export    | `nfm_tick` | `() -> ()` — one rAF pass; runs a game frame only when the sleep budget elapses |
//! | export    | `nfm_key` | `(code: u32, down: u32) -> ()` — Java `Event` codes from `window.rs`'s WATCH |
//! | export    | `nfm_mousedown` | `(x: u32, y: u32) -> ()` — the fase-111 click gate |
//! | export    | `nfm_frame_ptr` | `() -> *const u8` — the 550×400×3 RGB buffer |
//! | export    | `nfm_frame_len` | `() -> u32` — 660000 |
//! | export    | `nfm_audio_set_rate` | `(rate: u32) -> ()` — tell the bus the context output rate (called once at boot) |
//! | export    | `nfm_audio_buf_ptr` | `() -> *const i16` — address of the mono i16 staging buffer |
//! | export    | `nfm_audio_cap` | `() -> u32` — staging capacity in frames (16384) |
//! | export    | `nfm_audio` | `(ptr: *mut i16, frames: u32) -> ()` — mix the bus into wasm memory at `ptr` |
//! | export    | `nfm_audio_pulled` | `() -> u64` — mixed frames pulled so far (probe counter) |
//!
//! Audio is bridged in W2 through a main-thread `ScriptProcessorNode`
//! (see `imp.rs`): the bus honours any output rate, so JS pulls at the
//! context's native rate and upmixes the mono i16 mix to the context's
//! channels itself. The styled page is W3 (the probe page here is
//! deliberately plain).

#[cfg(target_arch = "wasm32")]
mod imp;

#[cfg(target_arch = "wasm32")]
pub use imp::*;
