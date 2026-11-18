//! `host` — the dependency-free platform seam.
//!
//! The real-time backends (a desktop window, a wasm canvas) implement
//! [`RealTimeHost`]; the game loop ([`Shell::run_realtime`]) is written
//! once against it. Input crosses the seam as [`InputEvent`]s carrying
//! the JAVA `Event` codes (1004/1005/1006/1007 arrows, 10/32/27, ASCII
//! letters) — every backend translates its platform's input into these,
//! so the transcription boundary is the code table, not the device.
//! Future variants (gamepad for the Deck, touch for wasm) join the enum,
//! not the trait.
//!
//! The seam itself is std-only (`thread::sleep` for the frame pace);
//! windowing and audio-drain backends live OUTSIDE core by design. (Core
//! does use `flate2` for the zipo archives; this module adds nothing.)

use std::time::Duration;

use crate::canvas::Canvas;
use crate::conto::Base;
use crate::medium::Medium;
use crate::shell::{Game, Shell};
use crate::xtgraphics::XtGraphics;

/// A platform input event, already translated to the Java `Event` codes
/// the handlers switch on (GameSparker.java:1519-1597).
///
/// - arrows: 1004 up, 1005 down, 1007 right, 1006 left
/// - 32 handbrake, 10/80/112/27 enter, 97/65 arrow-toggle, 77/109
///   music-mute toggle, 78/110 sound-mute toggle, 118/86 view cycle
/// - `MouseDown` carries the click pixel (GameSparker.java:300-310 sets
///   `xm`/`ym` from it; there is no mouseUp/mouseMove override — a second
///   press while one is armed is swallowed, and the per-menu-frame
///   `click()` conversion promotes 1 → 2).
///
/// Future variants (gamepad, touch) extend this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputEvent {
    KeyDown(i32),
    KeyUp(i32),
    MouseDown(i32, i32),
}

/// What a real-time backend provides: frame presentation, input polling,
/// and the wall clock. The desktop backend drives this with minifb, the
/// wasm backend with a web-sys canvas + `KeyboardEvent`, the tests with
/// a scripted rig.
pub trait RealTimeHost {
    /// Present the finished frame (the Canvas' RGB triples).
    fn present(&mut self, cv: &Canvas);
    /// Poll the next pending input event, if any.
    fn poll_event(&mut self) -> Option<InputEvent>;
    /// The wall clock in milliseconds.
    fn now_ms(&mut self) -> u64;
    /// The host's stop signal (the window's close button; the applet's
    /// stop() sets exwist the same way). Default: never — the headless
    /// test rig keeps this and exits via `exwist` instead.
    fn wants_exit(&mut self) -> bool {
        false
    }
    /// The audio output the host drives (`None` = silent). Default: none —
    /// the loop stays headless and every voice keeps its local cursor.
    fn audio(&mut self) -> Option<&mut dyn AudioSink> {
        None
    }
    /// One per-frame input pump, run after `present` and before the event
    /// drain. For hosts whose input sources need the app context: `fase`
    /// is the applet phase (`Shell.xt.fase`), which a context-dependent
    /// device — the gamepad's A-button route — reads as it folds its
    /// controls. Default: nothing; the headless test rig keeps today's
    /// shape.
    fn pump(&mut self, _fase: i32) {}
}

/// A pull-model audio sink: fill `out` with mixed i16 samples at `rate`
/// Hz. Implemented by [`crate::audiobus::AudioBus`]; the device backend
/// (next round) pulls through it from its output thread.
pub trait AudioSink {
    fn pull(&mut self, out: &mut [i16], rate: u32);
}

impl Shell {
    /// `GameSparker.keyDown` (GameSparker.java:1519-1591): the key-down
    /// if-chain over `game.ctl[0]`, gated on `!exwist`. Each arm is an
    /// independent `if`, like the source — including the toggles
    /// (arrace/mutem/mutes flip on both cases) and the `game.view` cycle
    /// (`view++; if view == 3 { view = 0 }`). The toggle if/else shape is
    /// the source's verbatim (hence the allow below).
    #[allow(clippy::needless_bool_assign)]
    pub fn key_down(&mut self, game: &mut Game, code: i32) {
        if !self.exwist {
            if code == 1004 {
                game.ctl[0].up = true;
            }

            if code == 1005 {
                game.ctl[0].down = true;
            }

            if code == 1007 {
                game.ctl[0].right = true;
            }

            if code == 1006 {
                game.ctl[0].left = true;
            }

            if code == 32 {
                game.ctl[0].handb = true;
            }

            if code == 10 || code == 80 || code == 112 || code == 27 {
                game.ctl[0].enter = true;
            }

            if code == 97 || code == 65 {
                if game.ctl[0].arrace {
                    game.ctl[0].arrace = false;
                } else {
                    game.ctl[0].arrace = true;
                }
            }

            if code == 77 || code == 109 {
                if game.ctl[0].mutem {
                    game.ctl[0].mutem = false;
                } else {
                    game.ctl[0].mutem = true;
                }
            }

            if code == 78 || code == 110 {
                if game.ctl[0].mutes {
                    game.ctl[0].mutes = false;
                } else {
                    game.ctl[0].mutes = true;
                }
            }

            if code == 118 || code == 86 {
                game.view += 1;
                if game.view == 3 {
                    game.view = 0;
                }
            }
        }
    }

    /// `GameSparker.keyUp` (GameSparker.java:266-285): release the held
    /// keys, gated on `!exwist`. No letter/enter handling — as sourced.
    pub fn key_up(&mut self, game: &mut Game, code: i32) {
        if !self.exwist {
            if code == 1004 {
                game.ctl[0].up = false;
            }

            if code == 1005 {
                game.ctl[0].down = false;
            }

            if code == 1007 {
                game.ctl[0].right = false;
            }

            if code == 1006 {
                game.ctl[0].left = false;
            }

            if code == 32 {
                game.ctl[0].handb = false;
            }
        }
    }

    /// `GameSparker.mouseDown` (GameSparker.java:300-310): arm the press
    /// with its coordinates, gated on `!exwist && mouses == 0`.
    pub fn mouse_down(&mut self, x: i32, y: i32) {
        if !self.exwist && self.mouses == 0 {
            self.xm = x;
            self.ym = y;
            self.mouses = 1;
        }
    }

    /// `GameSparker.lostFocus` (GameSparker.java:102-110): raise the
    /// focus-lost overlay flag, drop the mouse state and release every
    /// drive flag on the player's `Control`. The source returns `false`
    /// (the AWT consume contract); the port's handler is void. A backend's
    /// focus-out event calls this.
    pub fn lost_focus(&mut self, game: &mut Game) {
        if !self.exwist {
            self.lostfcs = true;
            self.mouses = 0;
            game.ctl[0].falseo();
        }
    }

    /// The real-time loop, written once: backends only implement
    /// [`RealTimeHost`]. Each pass stamps the loop-top clock, dispatches
    /// one frame, paces out the computed sleep, presents, then drains the
    /// pending input.
    ///
    /// Event-apply timing: the applet's events land between frames on the
    /// dispatch thread; polling AFTER the present (before the next
    /// frame's dispatch) gives the same net order — every event a frame
    /// observes arrived before that frame's dispatch. The `_` arm exists
    /// because [`InputEvent`] is `#[non_exhaustive]`.
    ///
    /// Exit paths: a set `exwist` breaks before the next dispatch; a
    /// host stop signal (`wants_exit` — the window close) flips `exwist`
    /// and runs ONE final `frame()` so `stopallnow()` silences every
    /// voice (GameSparker.java:1478-1486), then breaks. That final pass
    /// stamps both clocks together; its returned sleep is discarded —
    /// the loop exits.
    pub fn run_realtime(
        &mut self,
        game: &mut Game,
        bases: &[Base],
        xt: &mut XtGraphics,
        m: &mut Medium,
        cv: &mut Canvas,
        host: &mut dyn RealTimeHost,
    ) {
        loop {
            if self.exiting() {
                break;
            }
            if host.wants_exit() {
                self.exwist = true;
                let stamp = host.now_ms();
                self.frame(game, bases, xt, m, cv, stamp, stamp);
                break;
            }
            // First pass with a driving host: bind every loaded voice to
            // the bus. Per-voice handles are the latch (bound voices are
            // skipped), so late `loadmusic` builds still join the mix on
            // a later pass instead of staying silent. Headless hosts
            // return None here and nothing is ever bound.
            if host.audio().is_some()
                && let Some(bus) = &self.audio_bus
            {
                xt.bind_audio(bus);
            }
            let start = host.now_ms();
            let sleep = self.frame(game, bases, xt, m, cv, start, host.now_ms());
            if sleep > 0 {
                std::thread::sleep(Duration::from_millis(sleep as u64));
            }
            host.present(cv);
            // The host's input pump (the gamepad adapter lives here on the
            // window): its edges join the same queue the drain reads next.
            host.pump(xt.fase);
            while let Some(ev) = host.poll_event() {
                // The `_` arm is currently unreachable inside this crate
                // (the enum's variants are all listed) but stays: the
                // enum is non_exhaustive, so downstream backends matching
                // future variants (gamepad, touch) keep compiling.
                #[allow(unreachable_patterns)]
                match ev {
                    InputEvent::KeyDown(code) => self.key_down(game, code),
                    InputEvent::KeyUp(code) => self.key_up(game, code),
                    InputEvent::MouseDown(x, y) => self.mouse_down(x, y),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::Control;
    use crate::medium::Medium;

    fn rig() -> (Shell, Game) {
        let mut game = Game::new(vec![]);
        game.ctl[0] = Control::new();
        (Shell::new(), game)
    }

    /// The keyDown/keyUp table (GameSparker.java:1519-1597, 266-285):
    /// every code lands on its field, and keyUp clears only the held
    /// keys (no letter/enter release).
    #[test]
    fn key_down_up_mapping_table() {
        let (mut shell, mut game) = rig();
        shell.key_down(&mut game, 1004);
        assert!(game.ctl[0].up);
        shell.key_down(&mut game, 1005);
        assert!(game.ctl[0].down);
        shell.key_down(&mut game, 1007);
        assert!(game.ctl[0].right);
        shell.key_down(&mut game, 1006);
        assert!(game.ctl[0].left);
        shell.key_down(&mut game, 32);
        assert!(game.ctl[0].handb);
        shell.key_down(&mut game, 10);
        assert!(game.ctl[0].enter);

        shell.key_up(&mut game, 1004);
        assert!(!game.ctl[0].up);
        shell.key_up(&mut game, 1005);
        assert!(!game.ctl[0].down);
        shell.key_up(&mut game, 1007);
        assert!(!game.ctl[0].right);
        shell.key_up(&mut game, 1006);
        assert!(!game.ctl[0].left);
        shell.key_up(&mut game, 32);
        assert!(!game.ctl[0].handb);

        // keyUp has no letter/enter handling: enter stays held
        shell.key_up(&mut game, 10);
        shell.key_up(&mut game, 97);
        assert!(game.ctl[0].enter);
    }

    /// The toggles flip on BOTH cases (97/65, 77/109, 78/110) and the
    /// view cycles 0 → 1 → 2 → 0.
    #[test]
    fn toggles_flip_on_both_cases_and_view_wraps() {
        let (mut shell, mut game) = rig();

        shell.key_down(&mut game, 97);
        assert!(game.ctl[0].arrace);
        shell.key_down(&mut game, 65);
        assert!(!game.ctl[0].arrace);

        shell.key_down(&mut game, 77);
        assert!(game.ctl[0].mutem);
        shell.key_down(&mut game, 109);
        assert!(!game.ctl[0].mutem);

        shell.key_down(&mut game, 78);
        assert!(game.ctl[0].mutes);
        shell.key_down(&mut game, 110);
        assert!(!game.ctl[0].mutes);

        // 80/112/27 are further enter aliases
        shell.key_down(&mut game, 80);
        shell.key_down(&mut game, 112);
        shell.key_down(&mut game, 27);
        assert!(game.ctl[0].enter);

        shell.key_down(&mut game, 118);
        assert_eq!(game.view, 1);
        shell.key_down(&mut game, 86);
        assert_eq!(game.view, 2);
        shell.key_down(&mut game, 118);
        assert_eq!(game.view, 0, "the cycle wraps");
    }

    /// A set `exwist` swallows every handler (the `!this.exwist` gate).
    #[test]
    fn exwist_gate_swallows_all_input() {
        let (mut shell, mut game) = rig();
        shell.exwist = true;
        shell.key_down(&mut game, 1004);
        shell.key_down(&mut game, 32);
        shell.key_down(&mut game, 97);
        shell.key_down(&mut game, 118);
        shell.key_up(&mut game, 1004);
        shell.mouse_down(50, 60);
        assert!(!game.ctl[0].up);
        assert!(!game.ctl[0].handb);
        assert!(!game.ctl[0].arrace);
        assert_eq!(game.view, 0);
        assert_eq!((shell.mouses, shell.xm, shell.ym), (0, 0, 0));
    }

    /// `mouseDown`: coords land and the press arms; a second press while
    /// armed is swallowed (GameSparker.java:300-310).
    #[test]
    fn mouse_down_arms_once() {
        let (mut shell, mut game) = rig();
        shell.mouse_down(100, 200);
        assert_eq!((shell.mouses, shell.xm, shell.ym), (1, 100, 200));
        shell.mouse_down(300, 400);
        assert_eq!((shell.mouses, shell.xm, shell.ym), (1, 100, 200));

        // key handlers still run while the mouse is armed (separate gate)
        shell.key_down(&mut game, 1007);
        assert!(game.ctl[0].right);
    }

    /// A scripted headless host: a Vec-fed event source, a counting
    /// sink, a fake clock advancing 16ms per read.
    struct ScriptHost {
        events: Vec<InputEvent>,
        clock: u64,
        presented: usize,
    }

    impl RealTimeHost for ScriptHost {
        fn present(&mut self, _cv: &Canvas) {
            self.presented += 1;
        }

        fn poll_event(&mut self) -> Option<InputEvent> {
            if self.events.is_empty() {
                None
            } else {
                Some(self.events.remove(0))
            }
        }

        fn now_ms(&mut self) -> u64 {
            let t = self.clock;
            self.clock += 16;
            t
        }
    }

    #[test]
    fn realtime_pass_order_events_then_click() {
        let (mut shell, mut game) = rig();
        let mut xt = crate::xtgraphics::XtGraphics::new();
        // fase 10 (menu) keeps the dispatch cheap: maini + ctachm only.
        xt.fase = 10;
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases: Vec<crate::conto::Base> = vec![];
        let mut host = ScriptHost {
            events: vec![InputEvent::KeyDown(1007), InputEvent::MouseDown(111, 222)],
            clock: 1000,
            presented: 0,
        };

        // Pass one, in run_realtime's exact order: stamp, dispatch, pace,
        // present, then drain. The dispatch runs clean (nothing polled
        // yet); the drain queues both scripted events.
        let start = host.now_ms();
        let sleep = shell.frame(
            &mut game,
            &bases,
            &mut xt,
            &mut m,
            &mut cv,
            start,
            host.now_ms(),
        );
        // measured 30, 16ms elapsed: below the 15 floor, like the loop sees
        assert_eq!(sleep, shell.min_sleep);
        assert!(!game.ctl[0].right, "nothing polled before the drain");
        host.present(&cv);
        // The drain mirrors run_realtime's verbatim (same allow: the `_`
        // arm guards future non_exhaustive variants).
        #[allow(unreachable_patterns)]
        while let Some(ev) = host.poll_event() {
            match ev {
                InputEvent::KeyDown(code) => shell.key_down(&mut game, code),
                InputEvent::KeyUp(code) => shell.key_up(&mut game, code),
                InputEvent::MouseDown(x, y) => shell.mouse_down(x, y),
                _ => {}
            }
        }
        assert!(game.ctl[0].right, "KeyDown held (no KeyUp scripted)");
        assert_eq!((shell.mouses, shell.xm, shell.ym), (1, 111, 222));

        // pass two: the dispatch converts the armed click (1 → 2).
        let start = host.now_ms();
        shell.frame(
            &mut game,
            &bases,
            &mut xt,
            &mut m,
            &mut cv,
            start,
            host.now_ms(),
        );
        assert_eq!(shell.mouses, 2, "click() converted the press");
        host.present(&cv);
        assert_eq!(host.presented, 2);
    }

    /// The exit gate: a set `exwist` returns from `run_realtime` at once,
    /// draining nothing.
    ///
    /// (A mid-loop `exwist` flip from another thread would race the
    /// dispatch — the gate and the loop body are covered separately
    /// instead: here the immediate exit, and `frame_with_exwist` for the
    /// stopallnow-on-exit path.)
    #[test]
    fn realtime_loop_exits_without_running_when_flagged() {
        let (mut shell, mut game) = rig();
        let mut xt = crate::xtgraphics::XtGraphics::new();
        xt.fase = 10;
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases: Vec<crate::conto::Base> = vec![];
        let mut host = ScriptHost {
            events: vec![InputEvent::KeyDown(1007)],
            clock: 1000,
            presented: 0,
        };
        shell.exwist = true;
        shell.run_realtime(&mut game, &bases, &mut xt, &mut m, &mut cv, &mut host);
        assert!(shell.exiting());
        assert_eq!(host.presented, 0, "no frame ran");
        assert!(!game.ctl[0].right, "no event drained");
    }

    /// `frame()` with `exwist` set still runs `stopallnow` on the way
    /// out (GameSparker.java:1478-1486): an armed looping track stops.
    #[test]
    fn frame_with_exwist_runs_stopallnow() {
        let (mut shell, mut game) = rig();
        let mut xt = crate::xtgraphics::XtGraphics::new();
        xt.fase = 10;
        xt.loadedt[0] = true;
        let raw: &[u8] = include_bytes!("../../../assets/music/stage1.zipo");
        let z = crate::zipo::Zipo::read(raw).unwrap();
        let mut r = crate::radicalmod::RadicalMod::new(&z.entries[0].data, 350, 8400, 135);
        r.play();
        assert_eq!(r.posit(), 1191330, "the stage-1 stream really mixed");
        xt.stracks[0] = Some(r);
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases: Vec<crate::conto::Base> = vec![];
        shell.exwist = true;
        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 0, 16);
        assert!(!xt.stracks[0].as_ref().unwrap().playing);
        assert!(shell.exiting());
    }

    /// A scripted host that also signals stop: `wants_exit` flips once
    /// `presented` reaches the scripted count (the window-close path,
    /// headless).
    struct ClosingHost {
        events: Vec<InputEvent>,
        clock: u64,
        presented: usize,
        close_after: usize,
    }

    impl RealTimeHost for ClosingHost {
        fn present(&mut self, _cv: &Canvas) {
            self.presented += 1;
        }

        fn poll_event(&mut self) -> Option<InputEvent> {
            if self.events.is_empty() {
                None
            } else {
                Some(self.events.remove(0))
            }
        }

        fn now_ms(&mut self) -> u64 {
            let t = self.clock;
            self.clock += 16;
            t
        }

        fn wants_exit(&mut self) -> bool {
            self.presented >= self.close_after
        }
    }

    /// The host stop path: two presents run (the scripted KeyDown lands
    /// on the first), then the close flips `exwist`, one final frame
    /// runs `stopallnow()`, and the loop breaks without presenting it.
    #[test]
    fn wants_exit_breaks_after_final_frame() {
        let (mut shell, mut game) = rig();
        let mut xt = crate::xtgraphics::XtGraphics::new();
        xt.fase = 10;
        xt.loadedt[0] = true;
        let raw: &[u8] = include_bytes!("../../../assets/music/stage1.zipo");
        let z = crate::zipo::Zipo::read(raw).unwrap();
        let mut r = crate::radicalmod::RadicalMod::new(&z.entries[0].data, 350, 8400, 135);
        r.play();
        xt.stracks[0] = Some(r);
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases: Vec<crate::conto::Base> = vec![];
        let mut host = ClosingHost {
            events: vec![InputEvent::KeyDown(1007)],
            clock: 1000,
            presented: 0,
            close_after: 2,
        };
        shell.run_realtime(&mut game, &bases, &mut xt, &mut m, &mut cv, &mut host);
        assert!(shell.exiting(), "the close flipped exwist");
        assert_eq!(host.presented, 2, "the final frame is not presented");
        assert!(game.ctl[0].right, "the scripted press applied first");
        assert!(
            !xt.stracks[0].as_ref().unwrap().playing,
            "the final frame ran stopallnow"
        );
    }

    /// A sink the scripted host drives: lock the bus and pull through it.
    struct TestSink {
        bus: std::sync::Arc<std::sync::Mutex<crate::audiobus::AudioBus>>,
    }

    impl crate::host::AudioSink for TestSink {
        fn pull(&mut self, out: &mut [i16], rate: u32) {
            if let Ok(mut bus) = self.bus.lock() {
                bus.pull(out, rate);
            }
        }
    }

    /// A closing host that also drives audio: `audio()` hands out the
    /// test sink, `wants_exit` flips after `close_after` presents.
    struct ClosingAudioHost {
        events: Vec<InputEvent>,
        clock: u64,
        presented: usize,
        close_after: usize,
        sink: TestSink,
    }

    impl RealTimeHost for ClosingAudioHost {
        fn present(&mut self, _cv: &Canvas) {
            self.presented += 1;
        }

        fn poll_event(&mut self) -> Option<InputEvent> {
            if self.events.is_empty() {
                None
            } else {
                Some(self.events.remove(0))
            }
        }

        fn now_ms(&mut self) -> u64 {
            let t = self.clock;
            self.clock += 16;
            t
        }

        fn wants_exit(&mut self) -> bool {
            self.presented >= self.close_after
        }

        fn audio(&mut self) -> Option<&mut dyn crate::host::AudioSink> {
            Some(&mut self.sink)
        }
    }

    /// The audio latch: the first pass whose host drives audio binds every
    /// loaded voice to the shell's bus (once — later passes skip bound
    /// voices), and the shared cursor is the one `posit()` reads.
    #[test]
    fn audio_host_binds_voices_once() {
        use crate::radicalmod::{RadicalMod, SuperStream};
        use std::sync::{Arc, Mutex};

        let bus = Arc::new(Mutex::new(crate::audiobus::AudioBus::new()));
        let (mut shell, mut game) = rig();
        shell.bind_audio(Arc::clone(&bus));
        let mut xt = crate::xtgraphics::XtGraphics::new();
        xt.fase = 10;
        xt.loadedt[0] = true;
        let mut r = RadicalMod {
            stream: Some(SuperStream::new(Some(vec![10i8; 64]))),
            playing: false,
            rate: 8000,
            handle: None,
            #[cfg(test)]
            tag: String::new(),
        };
        r.play();
        xt.stracks[0] = Some(r);
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases: Vec<crate::conto::Base> = vec![];
        let mut host = ClosingAudioHost {
            events: vec![InputEvent::KeyDown(1007)],
            clock: 1000,
            presented: 0,
            close_after: 2,
            sink: TestSink {
                bus: Arc::clone(&bus),
            },
        };
        shell.run_realtime(&mut game, &bases, &mut xt, &mut m, &mut cv, &mut host);
        assert!(shell.exiting());
        assert_eq!(host.presented, 2);

        // bound on the first audio pass, exactly once
        let track = xt.stracks[0].as_ref().unwrap();
        assert!(track.handle.is_some(), "voice bound while audio drove");
        assert_eq!(bus.lock().unwrap().voice_count(), 1, "no duplicates");

        // the close path stopped everything through the bus
        assert!(!xt.stracks[0].as_ref().unwrap().playing);
        assert!(!bus.lock().unwrap().is_playing(0));

        // ... and the handle stays live afterwards: resume() re-arms the
        // bus voice, and a device-style pull moves the cursor posit()
        // reads (64 - 16 = 48)
        xt.stracks[0].as_mut().unwrap().resume();
        assert!(bus.lock().unwrap().is_playing(0));
        bus.lock().unwrap().pull(&mut [0i16; 16], 8000);
        assert_eq!(xt.stracks[0].as_ref().unwrap().posit(), 48);
    }

    /// No bus, no bind: a closing headless host terminates the loop with
    /// every voice still on its local cursor.
    #[test]
    fn no_bus_leaves_voices_local() {
        use crate::radicalmod::{RadicalMod, SuperStream};

        let (mut shell, mut game) = rig();
        let mut xt = crate::xtgraphics::XtGraphics::new();
        xt.fase = 10;
        xt.loadedt[0] = true;
        let mut r = RadicalMod {
            stream: Some(SuperStream::new(Some(vec![10i8; 64]))),
            playing: false,
            rate: 8000,
            handle: None,
            #[cfg(test)]
            tag: String::new(),
        };
        r.play();
        xt.stracks[0] = Some(r);
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases: Vec<crate::conto::Base> = vec![];
        let mut host = ClosingHost {
            events: vec![],
            clock: 1000,
            presented: 0,
            close_after: 1,
        };
        shell.run_realtime(&mut game, &bases, &mut xt, &mut m, &mut cv, &mut host);
        assert!(shell.exiting());
        let track = xt.stracks[0].as_ref().unwrap();
        assert!(track.handle.is_none(), "never bound without a bus");
        assert_eq!(track.posit(), 64, "local cursor untouched");
    }
}
