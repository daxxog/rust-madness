//! The wasm32-only host implementation. See `lib.rs` for the ABI table.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use nfm_core::audiobus::AudioBus;
use nfm_core::canvas::Canvas;
use nfm_core::conto::Base;
use nfm_core::cookies::CookieStore;
use nfm_core::host::InputEvent;
use nfm_core::medium::Medium;
use nfm_core::shell::{Game, Shell, build_cars};
use nfm_core::xtgraphics::XtGraphics;

const W: usize = 550;
const H: usize = 400;
const FRAME_LEN: usize = W * H * 3;

/// Mono i16 staging frames reserved for the ScriptProcessor callback. A
/// `createScriptProcessor(2048, ...)` asks for 2048 frames; 16384 leaves
/// generous headroom. Sized once at boot so `nfm_audio_buf_ptr` is stable
/// across callbacks (no realloc while JS holds a view into wasm memory).
const AUDIO_CAP_FRAMES: usize = 16384;

/// The clicknow panel is `fillRoundRect(125, 315, 300, 80, ...)` in
/// `crates/nfm-core/src/xtgraphics/screens.rs:11,63`; Enter targets its
/// centre so the key can stand in for the applet's click-to-start.
const CLICK_X: i32 = 125 + 300 / 2;
const CLICK_Y: i32 = 315 + 80 / 2;

// Raw wasm imports. No `#[link]` attribute: wasm32-unknown-unknown puts
// undefined `extern "C"` symbols in the `env` import module, which the probe
// page's `WebAssembly.instantiate` imports object supplies.
unsafe extern "C" {
    fn js_now_ms() -> f64;
    fn js_log(ptr: *const u8, len: usize);
}

fn now_ms() -> f64 {
    unsafe { js_now_ms() }
}

fn log(msg: &str) {
    unsafe { js_log(msg.as_ptr(), msg.len()) };
}

/// The one and only game instance (the applet model): boot state, the
/// pacing deadline, the input edge queue and the present buffer all live in
/// this single `OnceLock` cell. The `Vec`'s heap allocation gives
/// `nfm_frame_ptr` a stable address for the lifetime of the module.
struct Rig {
    bases: Vec<Base>,
    m: Medium,
    shell: Shell,
    xt: XtGraphics,
    game: Game,
    cv: Canvas,
    /// The mix bus the web audio bridge drains. Bound to the shell at boot
    /// (so `loadmusic`'s late voices join via the per-frame `bind_audio`
    /// below) and kept here so `nfm_audio` can pull it directly.
    bus: Arc<Mutex<AudioBus>>,
    /// Wall-clock ms when the next frame is due (0 → paint on first rAF).
    due_ms: f64,
    frames: u64,
    last_fase: i32,
}

struct App {
    rig: Option<Rig>,
    frame: Vec<u8>,
    queue: VecDeque<InputEvent>,
    boot_error: Option<String>,
    /// Mono i16 staging buffer. JS reads the mixed samples from here (its
    /// exported address is `nfm_audio_buf_ptr`); the i16→f32 conversion and
    /// mono→channels upmix happen in the glue.
    audio_buf: Vec<i16>,
    /// Output rate the bus mixes at — set from `AudioContext.sampleRate`
    /// at boot. `AudioBus::pull` honours any rate, so pulling at the
    /// context's native rate avoids a JS upsample pass entirely.
    audio_rate: u32,
    /// Frames of mixed mono audio actually pulled (verification counter).
    audio_pulled: u64,
}

static APP: OnceLock<Mutex<App>> = OnceLock::new();

fn app() -> &'static Mutex<App> {
    APP.get_or_init(|| {
        Mutex::new(App {
            rig: None,
            frame: vec![0; FRAME_LEN],
            queue: VecDeque::new(),
            boot_error: None,
            audio_buf: vec![0; AUDIO_CAP_FRAMES],
            audio_rate: 8000,
            audio_pulled: 0,
        })
    })
}

/// Duplicated host boot — mirror of `crates/nfm-render/src/main.rs:813-843`
/// (`play_mode`'s constructor spine): loadbase, the cookie store, `Medium`,
/// `Shell`, `XtGraphics` + `loadpak`, the carselect roster, `Game`, the
/// cookie override and the persistence latch, then the RGB canvas. Window,
/// gamepad and cpal setup (main.rs:845-891) are desktop-only and are
/// intentionally omitted — the cpal device is replaced by the JS
/// `ScriptProcessor` bridge below. The mix bus itself is created and bound
/// exactly like `play_mode` (main.rs:880-881). `nfm-core` is NOT refactored;
/// every call below is the same public API `play_mode` uses.
fn boot() -> Result<Rig, String> {
    let bases = nfm_core::stage::loadbase()?;
    let store = CookieStore::default_path();
    let m = Medium::new();
    let mut shell = Shell::new();
    let mut xt = XtGraphics::new();
    xt.loadpak();
    let cars = build_cars(&bases, &m);
    let mut game = Game::new(cars);
    game.cp.stage = 1;
    shell.read_cookies(&mut xt, &mut game.cp, &store);
    shell.store = Some(store);
    // The mix bus lives in Shell (voices bind lazily) and in the Rig (the
    // audio bridge drains it) — the same `Arc` both ends, as in play_mode.
    let bus = Arc::new(Mutex::new(AudioBus::new()));
    shell.bind_audio(Arc::clone(&bus));
    let cv = Canvas::new(m.w as usize, m.h as usize);
    Ok(Rig {
        bases,
        m,
        shell,
        xt,
        game,
        cv,
        bus,
        due_ms: 0.0,
        frames: 0,
        last_fase: 111,
    })
}

fn nonblack(frame: &[u8]) -> usize {
    frame
        .chunks_exact(3)
        .filter(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
        .count()
}

/// One-time boot. Safe to call more than once; later calls are no-ops.
#[unsafe(no_mangle)]
pub extern "C" fn nfm_init() {
    let mut a = app().lock().unwrap();
    if a.rig.is_some() || a.boot_error.is_some() {
        return;
    }
    log("nfm-web: boot");
    match boot() {
        Ok(rig) => {
            log(&format!(
                "nfm-web: boot ok — {} bases, fase {}, {} pack entries",
                rig.bases.len(),
                rig.xt.fase,
                nfm_core::pack::len()
            ));
            a.rig = Some(rig);
        }
        Err(e) => {
            log(&format!("nfm-web: boot FAILED: {e}"));
            a.boot_error = Some(e);
        }
    }
}

/// One rAF pass. Non-blocking by construction: the desktop loop's
/// `thread::sleep(sleep)` (`host.rs:246-248`) is replaced by a wall-clock
/// deadline. `js_now_ms` is the only clock — `Instant`/`SystemTime` both
/// panic on wasm32-unknown-unknown. rAF fires at ~60 Hz while the game paces
/// itself at ~18-25 Hz via `Shell::frame`'s returned sleep budget, so most
/// ticks just return and the last canvas is re-blitted by JS.
#[unsafe(no_mangle)]
pub extern "C" fn nfm_tick() {
    let mut a = app().lock().unwrap();
    // Destructure the guard once: field-splitting through `&mut *a` keeps
    // `rig`, `frame` and `queue` as independent `&mut` borrows.
    let App {
        rig: Some(rig),
        frame,
        queue,
        ..
    } = &mut *a
    else {
        return;
    };

    let now = now_ms();
    if now < rig.due_ms {
        return;
    }

    // run_realtime applies input AFTER present and BEFORE the next dispatch
    // (host.rs:249-265): events queued between ticks are all observed by
    // this frame. The `_` arm keeps the non_exhaustive enum future-proof.
    #[allow(unreachable_patterns)]
    while let Some(ev) = queue.pop_front() {
        match ev {
            InputEvent::KeyDown(code) => rig.shell.key_down(&mut rig.game, code),
            InputEvent::KeyUp(code) => rig.shell.key_up(&mut rig.game, code),
            InputEvent::MouseDown(x, y) => rig.shell.mouse_down(x, y),
            _ => {}
        }
    }

    // `run_realtime` binds loaded voices to the bus on every driving pass
    // (host.rs:239-243, idempotent per voice). This host drives `frame`
    // directly — rAF cannot host the blocking `run_realtime` loop — so we
    // replicate that latch here, including late `loadmusic` builds.
    rig.xt.bind_audio(&rig.bus);

    let start = now_ms() as u64;
    let sleep = rig.shell.frame(
        &mut rig.game,
        &rig.bases,
        &mut rig.xt,
        &mut rig.m,
        &mut rig.cv,
        start,
        now_ms() as u64,
    );

    // Present: copy the finished RGB canvas into the static buffer JS reads.
    if rig.cv.px.len() == frame.len() {
        frame.copy_from_slice(&rig.cv.px);
    }
    rig.frames += 1;

    if rig.xt.fase != rig.last_fase {
        log(&format!(
            "nfm-web: fase {} -> {} (frame {}, {} non-black px)",
            rig.last_fase,
            rig.xt.fase,
            rig.frames,
            nonblack(frame)
        ));
        rig.last_fase = rig.xt.fase;
    }

    // `min_sleep` floors the budget at 15 ms; `.max(1)` is belt-and-braces
    // against a busy loop should that floor ever change.
    rig.due_ms = now_ms() + sleep.max(1) as f64;
}

/// A game (Java `Event`) code from the JS key table; `down` is 1 on
/// keydown, 0 on keyup. Enter (code 10) additionally presses the clicknow
/// panel: the fase-111 gate reads ONLY the mouse latch (`shell.rs:219-233`),
/// so without this synthesis Enter could not dismiss the click-to-start
/// screen. Every other code is a plain key edge the shell consumes.
#[unsafe(no_mangle)]
pub extern "C" fn nfm_key(code: u32, down: u32) {
    let mut a = app().lock().unwrap();
    if a.rig.is_none() {
        return;
    }
    let code = code as i32;
    if down != 0 {
        if code == 10 {
            a.queue.push_back(InputEvent::MouseDown(CLICK_X, CLICK_Y));
        }
        a.queue.push_back(InputEvent::KeyDown(code));
    } else {
        a.queue.push_back(InputEvent::KeyUp(code));
    }
}

/// A canvas press in game coordinates; the fase-111 click gate.
#[unsafe(no_mangle)]
pub extern "C" fn nfm_mousedown(x: u32, y: u32) {
    let mut a = app().lock().unwrap();
    if a.rig.is_none() {
        return;
    }
    a.queue.push_back(InputEvent::MouseDown(x as i32, y as i32));
}

/// Pointer to the 550×400×3 RGB present buffer. JS makes a fresh
/// `Uint8Array(memory.buffer, ptr, len)` each blit in case wasm memory grew.
#[unsafe(no_mangle)]
pub extern "C" fn nfm_frame_ptr() -> *const u8 {
    app().lock().unwrap().frame.as_ptr()
}

/// Length of the present buffer (550 * 400 * 3 = 660000).
#[unsafe(no_mangle)]
pub extern "C" fn nfm_frame_len() -> u32 {
    FRAME_LEN as u32
}

// --- Web audio bridge ---------------------------------------------------
//
// The browser host uses a `ScriptProcessorNode` instead of an
// `AudioWorklet`: the worklet needs SharedArrayBuffer plus a separate
// module, and GitHub Pages cannot set the COOP/COEP headers SAB requires.
// ScriptProcessor is deprecated but universally supported, and its
// callback runs on the main thread — so it can call straight into this
// wasm instance with no ring buffer and no second memory. That is the
// simplest honest bridge for this host.

/// Tell the bridge the `AudioContext`'s output rate (Hz). Called once at
/// boot from JS. `AudioBus::pull` treats `rate` as the OUTPUT rate and
/// resamples every voice to it via its fractional cursor, so pulling at
/// the context's native rate needs no JS-side upsampling pass at all.
#[unsafe(no_mangle)]
pub extern "C" fn nfm_audio_set_rate(rate: u32) {
    app().lock().unwrap().audio_rate = rate;
}

/// Address of the mono i16 staging buffer JS reads after `nfm_audio`.
/// Allocated once at boot and never resized, so the address is stable for
/// the module's lifetime (no realloc while JS holds a view into memory).
#[unsafe(no_mangle)]
pub extern "C" fn nfm_audio_buf_ptr() -> *const i16 {
    app().lock().unwrap().audio_buf.as_ptr()
}

/// Capacity of the staging buffer in frames (JS clamps its request to it).
#[unsafe(no_mangle)]
pub extern "C" fn nfm_audio_cap() -> u32 {
    AUDIO_CAP_FRAMES as u32
}

/// Mix `frames` mono i16 samples from the bus into `ptr` — an address in
/// wasm linear memory, normally the value of `nfm_audio_buf_ptr()`. The
/// bus pull is ADDITIVE (layered pulls compose), so the staging buffer is
/// zeroed first, exactly like the desktop callback (nfm-render/audio.rs
/// `callback_staging_must_zero_before_pull`).
#[unsafe(no_mangle)]
// FFI contract: this `extern "C"` export is called from JS/WASM, which
// guarantees `ptr` points into its linear-memory audio buffer. Marking the
// export `unsafe` would reshape the C ABI surface; the pointer check below
// guards the only dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nfm_audio(ptr: *mut i16, frames: u32) {
    let mut a = app().lock().unwrap();
    let n = (frames as usize).min(a.audio_buf.len());
    if n == 0 {
        return;
    }
    for s in a.audio_buf[..n].iter_mut() {
        *s = 0;
    }
    let rate = a.audio_rate;
    // Clone the Arc so the guard's temporary does not keep `a` borrowed
    // while we write into `a.audio_buf` (disjoint fields, but the borrow
    // checker cannot see through the MutexGuard temporary).
    let bus = a.rig.as_ref().map(|rig| Arc::clone(&rig.bus));
    if let Some(bus) = bus
        && let Ok(mut bus) = bus.lock()
    {
        bus.pull(&mut a.audio_buf[..n], rate);
    }
    let src = a.audio_buf.as_ptr();
    if !std::ptr::eq(ptr, src) {
        // A caller-supplied different wasm address: copy the mix there.
        unsafe { std::ptr::copy_nonoverlapping(src, ptr, n) };
    }
    a.audio_pulled += n as u64;
}

/// Frames of mixed mono audio pulled so far — the probe counter.
#[unsafe(no_mangle)]
pub extern "C" fn nfm_audio_pulled() -> u64 {
    app().lock().unwrap().audio_pulled
}
