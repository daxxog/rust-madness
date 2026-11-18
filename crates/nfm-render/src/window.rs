//! `window` — the minifb desktop adapter implementing [`RealTimeHost`].
//!
//! [`Window`] owns a 550x400 minifb window and translates platform input
//! into the Java `Event` codes the shell switches on. Construction uses
//! `WindowOptions::default()` (1:1 `Scale::X1`, non-resizable) with
//! `set_target_fps(0)`: the loop self-paces via `frame()`'s returned
//! sleep, so minifb must not pace (its default is a 250 FPS cap; the old
//! `limit_update_rate` spelling is deprecated since 0.26).
//!
//! Channel order: the Canvas holds RGB triples, row-major top-down;
//! minifb's `update_with_buffer` takes `0RGB` words (`(r << 16) | (g <<
//! 8) | b`, upper byte ignored — verified against the 0.29 docs and the
//! doc example's `from_u8_rgb`).
//!
//! Mouse space: minifb reports window points (`mouseLocationOutsideOf-
//! EventStream`, y flipped to top-down), which coincide 1:1 with canvas
//! pixels at the default `Scale::X1` (scale factor 1 on every backend,
//! including Retina). If a later backend raises the scale, divide by the
//! factor (or use `get_unscaled_mouse_pos`, which skips the division).
//!
//! [`RealTimeHost`] for `nfm_core::host`; the pure pieces (key table,
//! edge detectors, buffer conversion) are free functions with tests.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use minifb::{Key, MouseButton, MouseMode, WindowOptions};
use nfm_core::canvas::Canvas;
use nfm_core::host::{AudioSink, InputEvent, RealTimeHost};

use crate::audio::CpalSink;

/// Canvas size: the applet's fixed 550x400 frame.
pub const WIDTH: usize = 550;
/// Canvas size: the applet's fixed 550x400 frame.
pub const HEIGHT: usize = 400;

/// The watched keys in poll order with their Java `Event` codes
/// (GameSparker.java:1519-1597: 1004/1005/1007/1006 arrows, 32 handbrake,
/// 10 enter, 27 escape; letters bind lowercase — the shell accepts both
/// cases for every letter binding, so the adapter emits the lowercase
/// member and case-insensitivity lives in the handler).
const WATCH: [(Key, i32); 12] = [
    (Key::Up, 1004),
    (Key::Down, 1005),
    (Key::Right, 1007),
    (Key::Left, 1006),
    (Key::Space, 32),
    (Key::Enter, 10),
    (Key::Escape, 27),
    (Key::A, 97),
    (Key::M, 109),
    (Key::N, 110),
    (Key::V, 118),
    (Key::P, 112),
];

/// Number of watched keys — the array shape [`key_edges`] takes. An adapter
/// that folds another device into the same `WATCH` rows (the gamepad) keeps
/// its down-state in this shape.
pub const NKEYS: usize = WATCH.len();

/// The `WATCH` index of a Java code, or `None` when the code is not a
/// watched key. The gamepad adapter maps its controls to these rows so the
/// keyboard's edge detector runs over both sources unchanged.
pub fn watch_index(code: i32) -> Option<usize> {
    WATCH.iter().position(|&(_, c)| c == code)
}

/// Pure edge detector over [`WATCH`]: `pressed[i]` is the current
/// down-state of `WATCH[i].0`. Returns the fired edges in table order —
/// down-edge → `KeyDown`, up-edge → `KeyUp` — and updates `prev` in
/// place, so holding emits nothing and a release-then-press across polls
/// re-fires exactly once.
pub fn key_edges(prev: &mut [bool; NKEYS], pressed: [bool; NKEYS]) -> Vec<InputEvent> {
    let mut out = Vec::new();
    for (i, &(_, code)) in WATCH.iter().enumerate() {
        if pressed[i] && !prev[i] {
            prev[i] = true;
            out.push(InputEvent::KeyDown(code));
        } else if !pressed[i] && prev[i] {
            prev[i] = false;
            out.push(InputEvent::KeyUp(code));
        }
    }
    out
}

/// Pure mouse edge: a button press becomes one `MouseDown` at the clamped
/// window point; holds and releases emit nothing (the shell's
/// `mouses == 0` gate swallows repeats anyway — this keeps the queue
/// quiet on top).
pub fn mouse_edge(prev_down: &mut bool, down: bool, pos: Option<(f32, f32)>) -> Option<InputEvent> {
    if down && !*prev_down {
        *prev_down = true;
        pos.map(|(x, y)| InputEvent::MouseDown(x as i32, y as i32))
    } else {
        if !down {
            *prev_down = false;
        }
        None
    }
}

/// Canvas RGB triples → minifb `0RGB` words.
pub fn canvas_to_u32(px: &[u8]) -> Vec<u32> {
    px.chunks_exact(3)
        .map(|t| ((t[0] as u32) << 16) | ((t[1] as u32) << 8) | t[2] as u32)
        .collect()
}

/// How many recent edges `/keys` keeps (newest first).
#[cfg(debug_assertions)]
const DEBUG_RECENT: usize = 32;

/// The desktop window: a minifb `Window` plus the edge-detector state,
/// the framebuffer scratch, and a present counter (the play loop reports
/// it). Reads input state fresh every poll — `update_with_buffer` (via
/// `present`) pumps the event queue each frame, which is what makes
/// `get_keys`/`get_mouse_down` current.
///
/// In debug builds the window also owns the shared snapshot the local debug
/// server serves (`/frame.bmp` = the latest presented canvas, `/keys` = the
/// WATCH state plus recent edges). Release builds have neither the field nor
/// the update code.
pub struct Window {
    window: minifb::Window,
    buf: Vec<u32>,
    prev_keys: [bool; NKEYS],
    prev_mouse_down: bool,
    pending: VecDeque<InputEvent>,
    presented: usize,
    audio: Option<CpalSink>,
    pad: Option<crate::gamepad::Gamepad>,
    #[cfg(debug_assertions)]
    debug: std::sync::Arc<crate::debugserve::DebugShare>,
    #[cfg(debug_assertions)]
    recent: VecDeque<String>,
    #[cfg(debug_assertions)]
    last_fase: i32,
}

impl Window {
    /// Open the 550x400 window, unpaced (the game loop paces itself).
    pub fn new(title: &str) -> Result<Self, minifb::Error> {
        let mut window = minifb::Window::new(title, WIDTH, HEIGHT, WindowOptions::default())?;
        // Default is a 250 FPS cap; 0 disables minifb-side pacing so the
        // frame() sleep alone sets the pace.
        window.set_target_fps(0);
        Ok(Self {
            window,
            buf: vec![0; WIDTH * HEIGHT],
            prev_keys: [false; NKEYS],
            prev_mouse_down: false,
            pending: VecDeque::new(),
            presented: 0,
            audio: None,
            pad: None,
            #[cfg(debug_assertions)]
            debug: std::sync::Arc::new(crate::debugserve::DebugShare::default()),
            #[cfg(debug_assertions)]
            recent: VecDeque::new(),
            #[cfg(debug_assertions)]
            last_fase: 111,
        })
    }

    /// Attach the device sink after a successful open. Kept separate so a
    /// failed open leaves the window silent-but-playable.
    pub fn set_audio(&mut self, sink: CpalSink) {
        self.audio = Some(sink);
    }

    /// Attach the gamepad adapter (or `None` for keyboard-only play — the
    /// adapter's own failed init is exactly that). Kept separate like
    /// [`Self::set_audio`]: the window is playable with either missing.
    pub fn set_gamepad(&mut self, pad: Option<crate::gamepad::Gamepad>) {
        self.pad = pad;
    }

    /// Minifb's close-box state; the play loop polls it each pass and
    /// treats close as the applet stop (sets `exwist`).
    pub fn is_open(&self) -> bool {
        self.window.is_open()
    }

    /// Frames presented so far (the play loop's session summary).
    pub fn presented(&self) -> usize {
        self.presented
    }
}

/// Debug-build only: the snapshot the local read-only server serves, and the
/// probe's one window affordance. None of this exists in a release binary.
#[cfg(debug_assertions)]
impl Window {
    /// The shared frame/keys snapshot (`debugserve::DebugServer` reads it).
    pub fn debug_share(&self) -> std::sync::Arc<crate::debugserve::DebugShare> {
        std::sync::Arc::clone(&self.debug)
    }

    /// Publish the opened audio route's summary for `/env`.
    pub fn debug_audio(&self, summary: String) {
        if let Ok(mut audio) = self.debug.audio.lock() {
            *audio = summary;
        }
    }

    /// Publish the pad inventory (names only) for `/env`.
    pub fn debug_pads(&self, names: Vec<String>) {
        if let Ok(mut pads) = self.debug.pads.lock() {
            *pads = names;
        }
    }

    /// The probe's skip affordance: a current Enter/Escape/left-click. The
    /// probe samples this every frame and only honours a fresh press.
    pub fn skip_requested(&self) -> bool {
        self.window.is_key_down(Key::Enter)
            || self.window.is_key_down(Key::Escape)
            || self.window.get_mouse_down(MouseButton::Left)
    }

    /// Record one emitted edge for `/keys` (folded code + human name).
    fn debug_note(&mut self, ev: &InputEvent) {
        let line = match ev {
            InputEvent::KeyDown(code) => format!("down  {code:<5} {}", key_name(*code)),
            InputEvent::KeyUp(code) => format!("up    {code:<5} {}", key_name(*code)),
            InputEvent::MouseDown(x, y) => format!("mouse down {x},{y}"),
            _ => return,
        };
        self.recent.push_front(line);
        self.recent.truncate(DEBUG_RECENT);
    }

    /// Refresh the `/keys` text: WATCH rows with their current down-state,
    /// then the most recent edges (newest first). Cheap enough per present.
    fn debug_sync(&mut self) {
        use std::fmt::Write as _;
        let mut s = String::new();
        let _ = writeln!(s, "fase {}", self.last_fase);
        let _ = writeln!(s, "WATCH (code, down)");
        for (i, &(key, code)) in WATCH.iter().enumerate() {
            let _ = writeln!(
                s,
                "  [{}] {:<7} {code}",
                if self.prev_keys[i] { "x" } else { " " },
                format!("{key:?}")
            );
        }
        let _ = writeln!(s, "recent edges (newest first)");
        if self.recent.is_empty() {
            let _ = writeln!(s, "  (none)");
        }
        for line in &self.recent {
            let _ = writeln!(s, "  {line}");
        }
        if let Ok(mut keys) = self.debug.keys.lock() {
            *keys = s;
        }
    }
}

/// The Java `Event` code's human name (the `/keys` display only).
#[cfg(debug_assertions)]
fn key_name(code: i32) -> &'static str {
    match code {
        1004 => "Up",
        1005 => "Down",
        1006 => "Left",
        1007 => "Right",
        32 => "Space/handbrake",
        10 => "Enter",
        27 => "Escape",
        97 => "A/arrace",
        109 => "M/music",
        110 => "N/sfx",
        118 => "V/view",
        112 => "P",
        _ => "?",
    }
}

impl RealTimeHost for Window {
    /// The gamepad's frame: one gilrs drain + one edge pass, queued with
    /// the keyboard's edges so the drain applies both before the next
    /// `Shell::frame` dispatch (a pad edge and a key edge for one code in
    /// one frame are just two edges of one code). `fase` routes the
    /// context-dependent A button.
    fn pump(&mut self, fase: i32) {
        #[cfg(debug_assertions)]
        {
            self.last_fase = fase;
        }
        // Borrow the pad alone, then note the folded edges: the debug
        // recorder takes `&mut self`.
        let evs = match self.pad.as_mut() {
            Some(pad) => pad.poll(fase),
            None => Vec::new(),
        };
        #[cfg(debug_assertions)]
        for ev in &evs {
            self.debug_note(ev);
        }
        self.pending.extend(evs);
        #[cfg(debug_assertions)]
        self.debug_sync();
    }

    fn present(&mut self, cv: &Canvas) {
        debug_assert_eq!((cv.w, cv.h), (WIDTH, HEIGHT));
        self.buf = canvas_to_u32(&cv.px);
        // A closed window errors here; the exit is owned by wants_exit
        // (polled each pass), so the error is dropped.
        let _ = self.window.update_with_buffer(&self.buf, WIDTH, HEIGHT);
        self.presented += 1;
        // Publish the just-presented pixels for `/frame.bmp`: reuse the
        // shared buffer when its shape matches, so the steady state is one
        // copy, not one allocation per frame.
        #[cfg(debug_assertions)]
        {
            if let Ok(mut frame) = self.debug.frame.lock() {
                match frame.as_mut() {
                    Some(shared) if shared.len() == cv.px.len() => {
                        shared.copy_from_slice(&cv.px);
                    }
                    _ => *frame = Some(cv.px.clone()),
                }
            }
            self.debug_sync();
        }
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        if let Some(ev) = self.pending.pop_front() {
            return Some(ev);
        }
        let down = self.window.get_keys();
        let pressed = WATCH.map(|(k, _)| down.contains(&k));
        let mut evs = key_edges(&mut self.prev_keys, pressed);
        let mouse_down = self.window.get_mouse_down(MouseButton::Left);
        let pos = self.window.get_mouse_pos(MouseMode::Clamp);
        if let Some(ev) = mouse_edge(&mut self.prev_mouse_down, mouse_down, pos) {
            evs.push(ev);
        }
        #[cfg(debug_assertions)]
        for ev in &evs {
            self.debug_note(ev);
        }
        self.pending.extend(evs);
        #[cfg(debug_assertions)]
        self.debug_sync();
        self.pending.pop_front()
    }

    fn now_ms(&mut self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn wants_exit(&mut self) -> bool {
        !self.is_open()
    }

    fn audio(&mut self) -> Option<&mut dyn AudioSink> {
        self.audio.as_mut().map(|s| s as &mut dyn AudioSink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Key→Java-code table, driven through the edge detector one
    /// key at a time: arrows, handbrake, enter, escape and the lowercase
    /// letter members (the shell accepts both cases per binding —
    /// covered in nfm-core's host tests). Completeness is structural:
    /// every WATCH row fires its code, in table order.
    #[test]
    fn key_table_covers_the_game_keys() {
        // WATCH index → expected Java code.
        let cases = [
            (0, 1004), // Up
            (1, 1005), // Down
            (2, 1007), // Right
            (3, 1006), // Left
            (4, 32),   // Space
            (5, 10),   // Enter
            (6, 27),   // Escape
            (7, 97),   // A
            (8, 109),  // M
            (9, 110),  // N
            (10, 118), // V
            (11, 112), // P
        ];
        assert_eq!(cases.len(), NKEYS);
        for (idx, code) in cases {
            let mut prev = [false; NKEYS];
            let mut pressed = [false; NKEYS];
            pressed[idx] = true;
            assert_eq!(
                key_edges(&mut prev, pressed),
                vec![InputEvent::KeyDown(code)]
            );
            // release fires the matching KeyUp
            assert_eq!(
                key_edges(&mut prev, [false; NKEYS]),
                vec![InputEvent::KeyUp(code)]
            );
        }
    }

    /// The `WATCH` lookup the gamepad adapter folds onto: every row's code
    /// maps back to its index, and a code the keyboard does not watch is
    /// `None` (the shell accepts upper-case `A`/65 too, but only the
    /// lowercase member is watched).
    #[test]
    fn watch_index_round_trips() {
        for (i, &(_, code)) in WATCH.iter().enumerate() {
            assert_eq!(watch_index(code), Some(i));
        }
        assert_eq!(watch_index(65), None);
        assert_eq!(watch_index(13), None);
    }

    fn down(keys: &[usize]) -> [bool; NKEYS] {
        let mut pressed = [false; NKEYS];
        for &i in keys {
            pressed[i] = true;
        }
        pressed
    }

    /// Edge transitions: press → one KeyDown; hold → silence; release →
    /// one KeyUp; re-press re-fires; multi-key presses drain in table
    /// order through the adapter queue.
    #[test]
    fn key_edges_fire_once_per_transition() {
        let mut prev = [false; NKEYS];
        // index 2 is Right/1007, index 7 is A/97
        assert_eq!(
            key_edges(&mut prev, down(&[2])),
            vec![InputEvent::KeyDown(1007)]
        );
        assert_eq!(key_edges(&mut prev, down(&[2])), vec![], "hold is silent");
        assert_eq!(
            key_edges(&mut prev, down(&[2, 7])),
            vec![InputEvent::KeyDown(97)],
            "only the new edge fires"
        );
        assert_eq!(
            key_edges(&mut prev, down(&[7])),
            vec![InputEvent::KeyUp(1007)],
            "release fires once, in table order"
        );
        assert_eq!(key_edges(&mut prev, down(&[])), vec![InputEvent::KeyUp(97)]);
        assert_eq!(key_edges(&mut prev, down(&[])), vec![]);
        assert_eq!(
            key_edges(&mut prev, down(&[2])),
            vec![InputEvent::KeyDown(1007)]
        );
    }

    /// Mouse edge: one MouseDown per press at the clamped point; holds
    /// and releases stay quiet; a press with no position still arms.
    #[test]
    fn mouse_edge_fires_once_per_press() {
        let mut prev = false;
        assert_eq!(mouse_edge(&mut prev, false, None), None);
        assert_eq!(
            mouse_edge(&mut prev, true, Some((111.7, 222.2))),
            Some(InputEvent::MouseDown(111, 222)),
            "truncating cast, like the source's (int)"
        );
        assert_eq!(
            mouse_edge(&mut prev, true, Some((5.0, 5.0))),
            None,
            "hold is silent"
        );
        assert_eq!(mouse_edge(&mut prev, false, None), None);
        assert_eq!(
            mouse_edge(&mut prev, true, None),
            None,
            "no position, still arms"
        );
        assert!(prev);
    }

    /// 2x2 canvas → the documented 0RGB words.
    #[test]
    fn buffer_conversion_packs_0rgb() {
        let px = [
            255, 0, 0, // red
            0, 255, 0, // green
            0, 0, 255, // blue
            18, 52, 86, // 0x123456
        ];
        assert_eq!(
            canvas_to_u32(&px),
            vec![0x00FF0000, 0x0000FF00, 0x000000FF, 0x00123456]
        );
    }
}
