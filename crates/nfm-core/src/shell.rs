//! Port of the `GameSparker.run()` per-frame dispatch
//! (GameSparker.java:578-660 the run prologue, 640-930 the menu fase
//! blocks, 926-1070 fase 0, 1072-1420 the replay/crash/result/pause
//! blocks plus the lost-focus overlay and the tail).
//!
//! `Shell::frame` is ONE pass of the loop body — not the `while (true)`
//! loop. The caller iterates; `testlocation()`, the `setCursor` calls
//! and `repaint` ride with the real-time shell, as does every audio
//! call. The wall-clock sleep the tail computes is RETURNED (the caller
//! sleeps it); the cookie jar behind the saves is `cookies::CookieStore`.
//!
//! Source-local mapping (the run-method locals): the replay/fade counter
//! (`n9`) is `Shell::hold`, the splash/crash-cam hold counter (`n10`) is
//! `Shell::boot`, the crash-cam sweep counter (`n11`) is
//! `Shell::sweeps`, the fase-176 framer hold (`n4`, cookie-less default
//! 80) is `Shell::loadhold`, the cookie run-once (`var37`) is
//! `Shell::saved`, the game-speed factor (`var12`) is `Shell::factor`,
//! the frame-time estimate (`var19`) is `Shell::measured`, the race-arm
//! (`var20`) is `Shell::racing`, the 10-window counter (`var21`) is
//! `Shell::sample` with its millisecond stamp (`var17`) in
//! `Shell::sample_at`, and the sleep floor (`var2`) is
//! `Shell::min_sleep`. The applet mouse state (`mouses`: 1 press,
//! 2 release) and the pointer (`xm`/`ym`) plus `lostfcs` are shell fields
//! too.
//!
//! The dispatch keeps the source's if-chain ORDER — every block is a
//! separate `if`, not a `match`, so a later block fires in the SAME frame
//! when an earlier one transitions into it (111→9, 9→10, -9→7, 5→176,
//! 2→1, 176→6, 6→0, -2→-3/-4, -1→-6, -3→-4, -4→-5, -6→-7, -8→-7).
//! (The task listing ordered 10 before 8 and 7 before -5; the source runs
//! 8 before 10 and -5 before 7 — the source wins.)
//!
//! Counter sharing (matches the source exactly): `hold` feeds
//! `fasereplay1`'s replay, `fasewasted2`/`fasewasted3`'s replay,
//! `faseresult4`'s fade and `fasecantreply`'s hold, and the -7 arm resets
//! it. `boot` doubles as the 111 (800) / 9 (200) / -9 (5) splash counter
//! AND the crash-cam `hold` param of `fasewasted2`/`fasewasted3` — one
//! local in the source, one field here. `Game` owns the race state the
//! Java keeps as run-method locals and applet fields (`scene`, the five
//! `Madness`/`Control` slots, checkpoints, recorder, trackers, the
//! carselect `cars` row, the `newcar` fresh-model baselines, the
//! countdown and `view`).

use std::sync::{Arc, Mutex};

use crate::audiobus::AudioBus;
use crate::canvas::Canvas;
use crate::checkpoints::{CheckPoints, PLAYERS};
use crate::conto::{Base, Conto, follow};
use crate::control::Control;
use crate::cookies::CookieStore;
use crate::gamesparker;
use crate::madness::Madness;
use crate::medium::Medium;
use crate::record::Record;
use crate::stage::{self, Scene};
use crate::trackers::Trackers;
use crate::xtgraphics::XtGraphics;

/// The applet shell state: mouse press/release, pointer, focus, and the
/// shared run-loop counters (`hold` = the source's replay/fade local,
/// `boot` = the splash/crash-cam hold local, `loadhold` = the fase-176
/// framer hold, `sweeps` = the crash-cam screening counter), the fps
/// adaptives (`factor`/`measured`/`racing`/`sample`/`sample_at`, with
/// the `min_sleep` floor), the exit flag (`exwist`), the cookie run-once
/// (`saved`), the persistence store (`store`: `None` disables saves),
/// and the mix bus (`audio_bus`: `None` stays headless).
pub struct Shell {
    /// Applet mouse state: 1 press, 2 release.
    pub mouses: i32,
    pub xm: i32,
    pub ym: i32,
    pub lostfcs: bool,
    /// The source's replay/fade local: fase -1/-2 replay, -4 fade, -8
    /// hold; the -7 arm resets it to 0.
    pub hold: i32,
    /// The source's splash local: 111 (800) / 9 (200) / -9 (5); also the
    /// crash-cam `hold` param of `fasewasted2`/`fasewasted3`.
    pub boot: i32,
    /// The fase-176 framer hold (cookie-less default 80).
    pub loadhold: i32,
    /// The crash-cam screening counter for `fasewasted3`.
    pub sweeps: i32,
    /// The fps-machine adaptives (GameSparker.java:610-640 run locals):
    /// `factor` (var12, the game-speed factor, 35.0 default),
    /// `measured` (var19, the frame-time estimate, 30.0 default),
    /// `racing` (var20, whether the race branch is armed),
    /// `sample` (var21, frames in the current 10-window) and `sample_at`
    /// (var17, the window's millisecond stamp).
    pub factor: f32,
    pub measured: f32,
    pub racing: bool,
    pub sample: i32,
    pub sample_at: u64,
    /// The sleep floor (GameSparker.java:585-588: `var2` is 5, or 15 when
    /// `sunytyp() != 0` — the standing newsun build ⇒ 15).
    pub min_sleep: i64,
    /// The exit flag (`exwist`): the driver stops the loop when set.
    pub exwist: bool,
    /// The cookie run-once (`var37`): the -5 arm saves once per visit.
    pub saved: bool,
    /// The persistence store; `None` no-ops every save.
    pub store: Option<CookieStore>,
    /// The mix bus shared with the device thread (`None` = headless:
    /// every voice keeps its local cursor). Set once via
    /// [`Shell::bind_audio`]; the device backend holds the other `Arc`.
    pub audio_bus: Option<Arc<Mutex<AudioBus>>>,
}

impl Shell {
    pub fn new() -> Shell {
        Shell {
            mouses: 0,
            xm: 0,
            ym: 0,
            lostfcs: false,
            hold: 0,
            boot: 0,
            loadhold: 80,
            sweeps: 0,
            factor: 35.0,
            measured: 30.0,
            racing: false,
            sample: 0,
            sample_at: 0,
            min_sleep: 15,
            exwist: false,
            saved: false,
            store: None,
            audio_bus: None,
        }
    }

    /// Store the mix bus the device thread pulls from. The bus itself is
    /// bound to voices lazily: `run_realtime` calls `XtGraphics::bind_audio`
    /// on the first pass whose host drives audio, so un-wired (headless)
    /// runs never touch it.
    pub fn bind_audio(&mut self, bus: Arc<Mutex<AudioBus>>) {
        self.audio_bus = Some(bus);
    }

    /// The `mouses` press/release conversion every menu arm runs after
    /// `ctachm` (GameSparker.java: e.g. 722-727): a released press clears,
    /// a fresh press arms.
    fn click(&mut self) {
        if self.mouses == 2 {
            self.mouses = 0;
        }
        if self.mouses == 1 {
            self.mouses = 2;
        }
    }

    /// The driver reads this after `frame()`: `true` means the applet
    /// asked out (`exwist`) — the caller breaks the loop. The source's
    /// `rd.dispose()`/`System.gc()`/`gamer.stop()` are applet lifecycle
    /// and ride with the real-time shell.
    pub fn exiting(&self) -> bool {
        self.exwist
    }

    /// The boot cookie reads (GameSparker.java:610-633): `unlocked`
    /// (1..=11 arms the stage; 11 = finale picks a random stage and
    /// clears `opselect` — the `opselect = 0` sits in the OUTER if, so
    /// it runs for every in-range cookie), `usercar` (0..=9 arms
    /// `sc[0]`), `gameprfact` (anything but −1 takes over `factor` and
    /// zeroes `loadhold`). `Math.random()` is the global slot:
    /// `xt.random()` reads `crate::random::math_random()`.
    /// Out-of-range cookies are ignored; `read` already defaults to −1.
    pub fn read_cookies(&mut self, xt: &mut XtGraphics, cp: &mut CheckPoints, store: &CookieStore) {
        let v = store.read("unlocked");
        if (1..=11).contains(&v) {
            xt.unlocked = v;
            if xt.unlocked != 11 {
                cp.stage = xt.unlocked;
            } else {
                cp.stage = (xt.random() * 11.0) as i32 + 1;
            }
            xt.opselect = 0;
        }

        let v = store.read("usercar");
        if (0..=9).contains(&v) {
            xt.sc[0] = v;
        }

        let v = store.read("gameprfact");
        if v != -1 {
            // The source re-reads the cookie (GameSparker.java:628);
            // transcribed verbatim (observable only if the jar changed
            // mid-read).
            self.factor = store.read("gameprfact") as f32;
            self.loadhold = 0;
        }
    }

    /// One pass of the run-loop dispatch over `xt.fase`. The caller owns
    /// the frame loop and supplies the canvas; `bases` is the loaded
    /// model row (`stage::loadbase`) for stage loads.
    ///
    /// Timing: `frame_start_ms` is the loop-top stamp (the source's
    /// `var25`), `now_ms` the post-dispatch stamp (the source's `var48`,
    /// taken after `playsounds`); tests inject synthetic values, the
    /// driver stamps the wall clock. Returns the computed sleep in ms
    /// (`round(measured) − (now − frame_start)`, floored at `min_sleep`);
    /// the caller sleeps it and breaks when `exiting()` flips.
    /// Eight params like the race dispatch it mirrors (`fase0` precedent).
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        game: &mut Game,
        bases: &[Base],
        xt: &mut XtGraphics,
        m: &mut Medium,
        cv: &mut Canvas,
        frame_start_ms: u64,
        now_ms: u64,
    ) -> i64 {
        // GameSparker.java:640-653 — fase 111, the click-to-start gate.
        if xt.fase == 111 {
            if self.mouses == 1 {
                self.boot = 800;
            }
            if self.boot < 800 {
                xt.clicknow(cv, m);
                self.boot += 1;
            } else {
                self.boot = 0;
                xt.fase = 9;
                self.mouses = 0;
                self.lostfcs = false;
            }
        }
        // GameSparker.java:654-665 — fase 9, the radicalplay splash.
        if xt.fase == 9 {
            if self.boot < 200 {
                xt.rad(cv, m, self.boot == 0);
                self.boot += 1;
            } else {
                self.boot = 0;
                xt.fase = 10;
                self.mouses = 0;
                game.ctl[0].falseo();
            }
        }
        // GameSparker.java:666-677 — fase -9, the white flash into the
        // car screen.
        if xt.fase == -9 {
            if self.boot < 5 {
                cv.fill([255, 255, 255]);
                self.boot += 1;
            } else {
                self.boot = 0;
                xt.fase = 7;
                self.mouses = 0;
            }
        }
        // GameSparker.java:678-688 — fase 8, credits.
        if xt.fase == 8 {
            xt.credits(cv, m, &mut game.ctl[0]);
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
        }
        // GameSparker.java:689-701 — fase 10, the main menu.
        if xt.fase == 10 {
            xt.maini(cv, &mut game.ctl[0]);
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
        }
        // GameSparker.java:702-712 — fase 11, instructions.
        if xt.fase == 11 {
            xt.inst(cv, &mut game.ctl[0]);
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
        }
        // GameSparker.java:713-733 — fase -5, the result screen. The
        // podium draws the freshly unlocked model out of the `cars` row
        // (see `finish`'s docs). The unlocked/gameprfact/usercar cookie
        // saves ride with persistence.
        if xt.fase == -5 {
            xt.finish(
                &mut game.cp,
                &mut game.cars,
                cv,
                m,
                &game.t,
                &mut game.ctl[0],
            );
            // GameSparker.java:729-733 — the result-screen saves, once
            // per visit (`var37`): the unlock bump when the newest stage
            // falls, then factor and car. `store == None` no-ops.
            if self.saved {
                if game.cp.stage == xt.unlocked
                    && xt.winner
                    && xt.unlocked != 11
                    && let Some(store) = self.store.as_mut()
                {
                    store.save("unlocked", &(xt.unlocked + 1).to_string());
                }
                if let Some(store) = self.store.as_mut() {
                    store.save("gameprfact", &(self.factor as i32).to_string());
                }
                if let Some(store) = self.store.as_mut() {
                    store.save("usercar", &xt.sc[0].to_string());
                }
                self.saved = false;
            }
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
        }
        // GameSparker.java:734-745 — fase 7, car select. The car contour
        // is the persistent `cars[sc[0]]` slot: the source mutates its
        // loadbase pool entry in place across frames (z/y/x/xz/zy/wzy
        // persist), and one slot per model mirrors that exactly.
        if xt.fase == 7 {
            let cart = xt.sc[0] as usize;
            xt.carselect(
                &mut game.ctl[0],
                &mut game.cars[cart],
                &game.mad[0],
                cv,
                m,
                &game.t,
                None,
            );
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
        }
        // GameSparker.java:746-756 — fase 6, the pre-race notice.
        if xt.fase == 6 {
            xt.musicomp(game.cp.stage, cv, m, &mut game.ctl[0]);
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
        }
        // GameSparker.java:769-773 — fase 5, music load. The first visit
        // saves the car cookie (`var37` arms the run-once).
        if xt.fase == 5 {
            xt.loadmusic(game.cp.stage, self.loadhold, cv, m);
            if !self.saved {
                if let Some(store) = self.store.as_mut() {
                    store.save("usercar", &xt.sc[0].to_string());
                }
                self.saved = true;
            }
        }
        // GameSparker.java:765-775 — fase 4, locked-stage notice.
        if xt.fase == 4 {
            xt.cantgo(cv, m, &mut game.ctl[0]);
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
        }
        // GameSparker.java:776-786 — fase 3, load failure.
        if xt.fase == 3 {
            xt.loadingfailed(game.cp.stage, cv, m, &mut game.ctl[0]);
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
        }
        // GameSparker.java:787-792 — fase 2, stage load: the loading
        // screen, then `loadstage`, then the slot-0 input clear.
        if xt.fase == 2 {
            xt.loadingstage(game.cp.stage, cv, m);
            if let Ok(mut scene) =
                stage::loadstage(game.cp.stage, bases, m, &mut game.t, &mut game.cp, xt)
            {
                // The post-parse tail (GameSparker.java:575-586): all
                // five controls reset against the checkpoints, each
                // car slot reset through `Madness.reseto` against the
                // fresh pool cars, the recorder primed, and the
                // fresh-model baselines captured per roster model
                // (the source rebuilds from its loadbase pool entry;
                // the carselect-persistent `cars` row is that pool).
                // GameSparker.java:563-566 — the Control reset walk.
                stage::reset_controls(&game.cp, &mut game.ctl);
                for (i, (md, o)) in game.mad.iter_mut().zip(scene.pool.iter_mut()).enumerate() {
                    md.reseto(xt.sc[i] as usize, o, &mut game.cp);
                }
                game.rpd.reset(&scene.pool, m);
                for i in 0..PLAYERS {
                    let model = xt.sc[i] as usize;
                    game.newcar[model] = Some(Conto::clone_from(&game.cars[model], m, 0, 0, 0, 0));
                }
                game.starcnt = xt.starcnt;
                game.view = 0;
                game.scene = Some(scene);
            }
            game.ctl[0].falseo();
        }
        // GameSparker.java:793-851 — fase 1, the stage-select flyover:
        // backdrop, the flyover camera step, the pool draw over
        // pool[5..notb), the mouse arm, then the select screen. The
        // conversion runs between `ctachm` and `stageselect` as in the
        // source (stageselect reads keys, not the mouse state).
        if xt.fase == 1
            && let Some(scene) = game.scene.as_mut()
        {
            xt.trackbg(cv, m);
            gamesparker::fase1(scene, cv, m, &game.t, &game.cp);
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
            xt.stageselect(cv, &mut game.cp, m, &mut game.ctl[0]);
        }
        // GameSparker.java:852-884 — fase 176, the load countdown: the
        // race-style sorted draw over pool[0..nob), the player follow
        // camera, the progress screen, then the hold countdown into the
        // pre-race notice. The track `play()` rides with audio.
        if xt.fase == 176
            && let Some(scene) = game.scene.as_mut()
        {
            m.d(cv);
            let nob = scene.nob;
            gamesparker::draw_sorted(scene, cv, m, &game.t, 0, nob);
            follow(m, &scene.pool[0], 0);
            xt.framer(game.cp.stage, cv, m);
            if self.loadhold != 0 {
                self.loadhold -= 1;
            } else {
                game.ctl[0].enter = false;
                game.ctl[0].handb = false;
                // GameSparker.java:917-919 — the track starts under the
                // pre-race notice when armed.
                if xt.loadedt[(game.cp.stage - 1) as usize]
                    && let Some(r) = xt.stracks[(game.cp.stage - 1) as usize].as_mut()
                {
                    r.play();
                }
                xt.fase = 6;
            }
        }
        // GameSparker.java:885-1029 — fase 0, the race frame, with the
        // audio hub's state tail. The countdown threads through the game
        // (the port's `starcnt` param); `xt.starcnt` re-syncs inside.
        if xt.fase == 0
            && let Some(scene) = game.scene.as_mut()
        {
            // the source's counter is `xtGraphics.starcnt` (the tests and
            // the replay paths write it directly), so it seeds the local.
            let mut starcnt = xt.starcnt;
            gamesparker::fase0(
                scene,
                &mut game.mad,
                &mut game.ctl,
                &mut game.cp,
                cv,
                m,
                &game.t,
                &mut game.rpd,
                &mut starcnt,
                game.view,
                &game.newcar,
                xt,
            );
            game.starcnt = starcnt;
            if xt.starcnt == 35 {
                // GameSparker.java:924-926 — the starcnt-35 landing clears
                // the mouse state next to the vert/adv/vxz reset; fase0
                // has no mouse access, so the shell does it. The landing
                // leaves starcnt at 35 (the countdown resumes at 34 next
                // frame), so this also fires on skip-clamped frames —
                // idempotent either way.
                self.mouses = 0;
            }
            // GameSparker.java:1045-1049 — a pressed mouse in the race
            // block synthesizes enter for the next frame's stat/continue
            // routing (the source sits after the stat draws).
            if self.mouses == 1 {
                game.ctl[0].enter = true;
                self.mouses = 0;
            }
        }
        // GameSparker.java:1030-1071 + vineflower 1156-1166 — fase -1,
        // the reply film. The port runs the seed, draw, skip arm, fix
        // chain and the counter increment with its 300 → 0 wrap; the
        // shell owns the block tail: a post-call `hold` of 0 means the
        // film wrapped this frame (the only way back to 0), so the track
        // stop (audio slice) hands to fase -6, otherwise the reply bar
        // paints — then the camera swings onto the player car either way.
        // The -1 → -6 transition chains into the -6 arm in the same frame.
        if xt.fase == -1
            && let Some(scene) = game.scene.as_mut()
        {
            gamesparker::fasereplay1(
                scene,
                &mut game.mad,
                &mut game.ctl,
                cv,
                m,
                &game.t,
                &mut game.rpd,
                &mut self.hold,
                &game.newcar,
            );
            if self.hold == 0 {
                // GameSparker.java:1156-1158 — the film wrap stops the
                // stage track when armed (a missed site the music slice
                // left as a comment; wired here).
                if xt.loadedt[(game.cp.stage - 1) as usize]
                    && let Some(r) = xt.stracks[(game.cp.stage - 1) as usize].as_mut()
                {
                    r.stop();
                }
                xt.fase = -6;
            } else {
                xt.replyn(cv, m);
            }
            crate::conto::around(m, &scene.pool[0], false);
        }
        // GameSparker.java:1072-1089 — fase -2, the wasted handoff.
        if xt.fase == -2 {
            gamesparker::fasewasted2(
                m,
                cv,
                &game.rpd,
                &mut self.hold,
                &mut self.boot,
                &mut self.sweeps,
                xt,
            );
        }
        // GameSparker.java:1090-1157 — fase -3, the crash-cam film.
        if xt.fase == -3
            && let Some(scene) = game.scene.as_mut()
        {
            gamesparker::fasewasted3(
                scene,
                &mut game.mad,
                &mut game.ctl,
                cv,
                m,
                &game.t,
                &mut game.rpd,
                &mut self.hold,
                &mut self.boot,
                &mut self.sweeps,
                xt,
                &game.newcar,
            );
        }
        // GameSparker.java:1158-1176 — fase -4, the result fade.
        if xt.fase == -4 {
            gamesparker::faseresult4(xt, cv, m, &game.cp, &mut self.hold, &mut game.ctl[0]);
        }
        // GameSparker.java:1177-1181 — fase -6, the pause snapshot.
        if xt.fase == -6 {
            xt.pauseimage(cv, m);
            xt.fase = -7;
            self.mouses = 0;
        }
        // GameSparker.java:1182-1197 — fase -7, the pause menu.
        if xt.fase == -7 {
            xt.pausedgame(cv, game.cp.stage, &mut game.ctl[0], &game.rpd);
            if self.hold != 0 {
                self.hold = 0;
            }
            xt.ctachm(self.xm, self.ym, self.mouses, &mut game.ctl[0]);
            self.click();
        }
        // GameSparker.java:1198-1210 — fase -8, the no-reply notice. The
        // mouse-state reset on the transition back to -7 belongs to the
        // shell loop (see `fasecantreply`'s docs).
        if xt.fase == -8 {
            gamesparker::fasecantreply(xt, cv, m, &mut self.hold, &mut game.ctl[0], self.mouses);
            if xt.fase == -7 {
                self.mouses = 0;
            }
        }
        // GameSparker.java:1211-1229 — the lost-focus overlay.
        if self.lostfcs && xt.fase != 176 && xt.fase != 111 {
            xt.nofocus(cv, m);
            if xt.fase == 0 {
                game.ctl[0].enter = true;
            }
            if self.mouses == 1 || self.mouses == 2 {
                self.lostfcs = false;
                if xt.fase == 10 {
                    xt.flipo = 0;
                }
                if xt.fase == 11 && xt.flipo != 0 {
                    xt.flipo -= 1;
                }
                if xt.fase == -7 {
                    cv.draw_image(&xt.fleximg, 0, 0);
                }
            }
        }
        // GameSparker.java:1402 — the audio hub's state tail runs every
        // loop iteration after the fase dispatch; the fps machine reads
        // `now_ms` as its end-of-dispatch stamp (the source's `var48`).
        xt.playsounds(&game.mad[0], &game.ctl[0], game.cp.stage, m);
        // GameSparker.java:1403-1487 — the fps machine. Menu branch
        // (fase ∉ {0, −1, −3}): leaving the race folds `measured` back
        // into `factor`; the 176 arm re-baselines on `loadhold == 79`
        // (`measured = factor`, fresh window) and takes `factor` over on
        // `loadhold == 1`; otherwise the plain 400ms/±3.5 windows run.
        // Race branch (fase ∈ {0, −1, −3}): arming copies `factor` into
        // `measured`; the 10-frame 530ms/±0.5 windows run, and a settled
        // countdown (`starcnt == 0`) eases the fade toward the estimate.
        if xt.fase != 0 && xt.fase != -1 && xt.fase != -3 {
            if self.racing {
                self.factor = self.measured;
                self.racing = false;
                self.sample = 0;
            }

            if self.loadhold != 0 && xt.fase == 176 {
                if self.loadhold == 79 {
                    self.measured = self.factor;
                    self.sample_at = now_ms;
                    self.sample = 0;
                }

                if self.sample == 10 {
                    if now_ms.wrapping_sub(self.sample_at) < 530 {
                        self.measured += 5.0;
                    } else {
                        self.measured -= 5.0;
                        if self.measured < 5.0 {
                            self.measured = 5.0;
                        }
                    }

                    self.sample_at = now_ms;
                    self.sample = 0;
                } else {
                    self.sample += 1;
                }

                if self.loadhold == 1 {
                    self.factor = self.measured;
                }
            } else if self.sample == 10 {
                if now_ms.wrapping_sub(self.sample_at) < 400 {
                    self.measured += 3.5;
                } else {
                    self.measured -= 3.5;
                    if self.measured < 5.0 {
                        self.measured = 5.0;
                    }
                }

                self.sample_at = now_ms;
                self.sample = 0;
            } else {
                self.sample += 1;
            }
        } else {
            if !self.racing {
                self.measured = self.factor;
                self.racing = true;
                self.sample = 0;
            }

            if self.sample == 10 {
                if now_ms.wrapping_sub(self.sample_at) < 530 {
                    self.measured += 0.5;
                } else {
                    self.measured -= 0.5;
                    if self.measured < 5.0 {
                        self.measured = 5.0;
                    }
                }

                if xt.starcnt == 0 {
                    m.adjstfade(self.measured);
                }

                self.sample_at = now_ms;
                self.sample = 0;
            } else {
                self.sample += 1;
            }
        }

        // GameSparker.java:1478-1486 — the applet teardown rides with the
        // real-time shell (`rd.dispose()`/`System.gc()`/`gamer.stop`);
        // the port stops the voices and reports through `exiting()`.
        if self.exwist {
            xt.stopallnow();
        }

        // GameSparker.java:1488-1494 — `round(measured) − elapsed`,
        // floored at the sleep minimum; the caller sleeps it.
        let sleep = self.measured.round() as i64 - now_ms.wrapping_sub(frame_start_ms) as i64;
        if sleep < self.min_sleep {
            self.min_sleep
        } else {
            sleep
        }
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

/// The race state the Java keeps as run-method locals and applet fields:
/// the loaded scene (absent until the first fase-2 load), the five car
/// slots, checkpoints, recorder, shared trackers, the carselect `cars`
/// row, the `newcar` fresh-model baselines (the source's 43-entry pool,
/// indexed by car model), the countdown and the camera view.
pub struct Game {
    pub scene: Option<Scene>,
    pub mad: [Madness; PLAYERS],
    pub ctl: [Control; PLAYERS],
    pub cp: CheckPoints,
    pub rpd: Record,
    pub t: Trackers,
    /// The 10 carselect slots, built once from `bases[0..10]` — the
    /// source's loadbase pool entries the car screen mutates in place.
    pub cars: Vec<Conto>,
    /// Fresh-model baselines per car model id (`mad.cn`), cloned at the
    /// loadstage tail for the five roster models.
    pub newcar: Vec<Option<Conto>>,
    /// The countdown the port threads through `fase0` (`xt.starcnt`
    /// re-syncs inside the call); reset to 90 by the loadstage tail.
    pub starcnt: i32,
    /// The camera view (`GameSparker.view`): 0 follow, 1 around, 2 watch.
    pub view: usize,
}

impl Game {
    /// `cars` must hold the 10 model slots (see `build_cars`); the scene
    /// arrives with the first fase-2 load.
    pub fn new(cars: Vec<Conto>) -> Game {
        Game {
            scene: None,
            mad: [
                Madness::new(0),
                Madness::new(1),
                Madness::new(2),
                Madness::new(3),
                Madness::new(4),
            ],
            ctl: [
                Control::new(),
                Control::new(),
                Control::new(),
                Control::new(),
                Control::new(),
            ],
            cp: CheckPoints::new(),
            rpd: Record::new(),
            t: Trackers::new(),
            cars,
            newcar: (0..43).map(|_| None).collect(),
            starcnt: 90,
            view: 0,
        }
    }
}

/// Build the 10 carselect slots from the loaded model row. The contours
/// park at the origin; the car screen re-poses its slot every frame. A
/// scratch `Trackers` takes the placement registrations so the race
/// trackers stay clean (the source's loadbase entries never touch the
/// stage tracker pool either).
pub fn build_cars(bases: &[Base], m: &Medium) -> Vec<Conto> {
    let mut scratch = Trackers::new();
    (0..10)
        .map(|i| Conto::place(&bases[i], m, &mut scratch, 0, 0, 0, 0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::medium::Medium;

    /// The canonical applet walk, boot to first race frame: clicknow skip
    /// → rad ×200 → main menu (New Game) → instructions ×3 → car select →
    /// fase-2 stage-1 load → stage select → loadmusic → 176 countdown →
    /// pre-race notice → first race frame. Asserts the fase at every
    /// handoff; the stages/models archives resolve through the
    /// workspace-root-or-crate probe, like `XtGraphics::loadpak`.
    #[test]
    fn walk_boot_to_first_race_frame() {
        let bases = crate::stage::loadbase().expect("loadbase over models.zipo");
        assert_eq!(bases.len(), 43);
        let mut m = Medium::new();
        let cars = build_cars(&bases, &m);
        assert_eq!(cars.len(), 10);
        let mut game = Game::new(cars);
        let mut shell = Shell::new();
        let mut xt = XtGraphics::new();
        xt.loadpak();
        assert_eq!(xt.fase, 111);
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let mut seen: Vec<i32> = vec![xt.fase];
        // Synthetic 16ms frame clock (the fps machine still runs off
        // these stamps; the sleep it returns is discarded headlessly).
        let mut clock: u64 = 0;

        macro_rules! step {
            () => {{
                let start = clock;
                clock += 16;
                let _sleep = shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, start, clock);
                seen.push(xt.fase);
            }};
        }

        // clicknow paints; a press skips straight into the splash.
        step!();
        assert_eq!((xt.fase, shell.boot), (111, 1));
        shell.mouses = 1;
        step!();
        assert_eq!((xt.fase, shell.boot, shell.mouses), (9, 1, 0));

        // the splash runs 200 frames, then the main menu.
        for _ in 0..200 {
            step!();
        }
        assert_eq!((xt.fase, shell.boot), (10, 0));

        // New Game: up once (opselect 1 → 0), then enter.
        game.ctl[0].up = true;
        step!();
        assert_eq!((xt.fase, xt.opselect), (10, 0));
        game.ctl[0].enter = true;
        step!();
        assert_eq!((xt.fase, xt.oldfase), (11, 7));

        // instructions: three enters page through to the car screen.
        for _ in 0..3 {
            game.ctl[0].enter = true;
            step!();
        }
        assert_eq!((xt.fase, xt.flipo), (7, 0));

        // car select: the default slot 0 is playable, enter loads stage 1
        // (fase 2 → 1 chains in the same frame).
        game.ctl[0].enter = true;
        step!();
        assert_eq!(xt.fase, 1);
        let scene = game.scene.as_ref().expect("fase 2 builds the scene");
        assert_eq!(scene.nob, scene.pool.len());
        assert!(scene.notb > 5 && scene.notb <= scene.nob);
        assert_eq!(game.cp.name, "The Introductory Stage");
        assert_eq!(shell.loadhold, 80);

        // stage select: enter arms the music load (the fase-5 arm runs
        // before fase 1 in the chain, so loadmusic fires next frame).
        game.ctl[0].enter = true;
        step!();
        assert_eq!((xt.fase, shell.loadhold), (5, 80));

        // loadmusic lands fase 176 (hold 80 → 79 in the chained arm), then
        // the countdown wears on to the pre-race notice.
        step!();
        assert_eq!((xt.fase, shell.loadhold), (176, 79));
        let mut guard = 0;
        while xt.fase == 176 && guard < 500 {
            step!();
            guard += 1;
        }
        assert_eq!((xt.fase, shell.loadhold), (6, 0));

        // the notice: enter starts the race, frame 1 paints in-chain.
        game.ctl[0].enter = true;
        step!();
        assert_eq!(xt.fase, 0);
        assert_eq!(game.starcnt, 89);
        let ink = cv.px.chunks_exact(3).filter(|p| *p != [0u8, 0, 0]).count();
        assert!(ink > 1000, "race frame 1 paints, got {ink} ink px");

        // one-line fase trace for the report.
        seen.dedup();
        println!("walk fase trace: {seen:?}");
    }

    /// A missing stage diverts to the load-failure screen, like the
    /// source's catch arm.
    #[test]
    fn loadstage_missing_stage_fails_to_3() {
        let bases = crate::stage::loadbase().expect("loadbase over models.zipo");
        let mut m = Medium::new();
        let mut t = Trackers::new();
        let mut cp = CheckPoints::new();
        let mut xt = XtGraphics::new();
        xt.loadpak();
        xt.fase = 2;
        let err = match crate::stage::loadstage(99, &bases, &mut m, &mut t, &mut cp, &mut xt) {
            Ok(_) => panic!("stage 99 has no file"),
            Err(e) => e,
        };
        assert_eq!(xt.fase, 3);
        assert!(err.contains("99"), "unexpected error: {err}");
    }

    fn cookie_store(name: &str) -> (CookieStore, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("nfm-shell-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        (CookieStore::new(&path), path)
    }

    /// `read_cookies` (GameSparker.java:610-633): the clamps and the
    /// takeovers, including the finale's random stage.
    #[test]
    fn read_cookies_clamps_and_takeover() {
        let (mut store, path) = cookie_store("read");
        let mut shell = Shell::new();
        let mut xt = XtGraphics::new();
        let mut cp = CheckPoints::new();

        // out-of-range cookies are ignored (stale -1 defaults hold)
        store.save("unlocked", "0");
        store.save("usercar", "10");
        shell.read_cookies(&mut xt, &mut cp, &store);
        assert_eq!(xt.unlocked, 1, "default unlocked untouched");
        assert_eq!(xt.sc[0], 0, "default car untouched");
        assert_eq!(shell.factor, 35.0);
        assert_eq!(shell.loadhold, 80);

        // in-range cookies arm
        store.save("unlocked", "5");
        store.save("usercar", "9");
        store.save("gameprfact", "42");
        shell.read_cookies(&mut xt, &mut cp, &store);
        assert_eq!(xt.unlocked, 5);
        assert_eq!(cp.stage, 5);
        assert_eq!(xt.opselect, 0);
        assert_eq!(xt.sc[0], 9);
        assert_eq!(shell.factor, 42.0);
        assert_eq!(shell.loadhold, 0, "gameprfact zeroes the hold");

        // the finale (11) picks a random stage but still clears opselect
        store.save("unlocked", "11");
        shell.read_cookies(&mut xt, &mut cp, &store);
        assert_eq!(xt.unlocked, 11);
        assert!((1..=11).contains(&cp.stage), "finale stage {}", cp.stage);
        assert_eq!(xt.opselect, 0);

        // garbage parses to -1 and is ignored like a missing cookie
        store.save("unlocked", "lots");
        xt.unlocked = 3;
        shell.read_cookies(&mut xt, &mut cp, &store);
        assert_eq!(xt.unlocked, 3);
        let _ = std::fs::remove_file(&path);
    }

    /// The 176 fps arm (GameSparker.java:1422-1446): the `loadhold == 79`
    /// baseline pins `measured = factor` with a fresh window, then the
    /// 10-frame 530ms windows climb by 5.
    #[test]
    fn fps_machine_176_baseline_and_window() {
        let mut shell = Shell::new();
        let mut game = Game::new(vec![]);
        let mut xt = XtGraphics::new();
        xt.fase = 176;
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases = crate::stage::loadbase().expect("loadbase");

        // baseline frame: measured = factor, fresh window, no elapsed
        shell.loadhold = 79;
        shell.factor = 35.0;
        shell.measured = 30.0;
        let sleep = shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 1000, 1000);
        assert_eq!(shell.measured, 35.0);
        assert_eq!(shell.sample, 1);
        assert_eq!(shell.sample_at, 1000);
        assert_eq!(sleep, 35, "round(35) - 0 elapsed");

        // the dispatch arm owns the hold countdown (it needs a scene, so
        // the headless rig steps it by hand past the one-shot baseline)
        shell.loadhold = 78;

        // nine quiet frames fill the window ...
        for i in 1..10 {
            let _sleep = shell.frame(
                &mut game,
                &bases,
                &mut xt,
                &mut m,
                &mut cv,
                1000 + i * 16,
                1000 + (i + 1) * 16,
            );
            assert_eq!(shell.sample, (i + 1) as i32);
        }
        // ... the tenth frame closes it under 530ms: measured += 5
        let sleep = shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 1160, 1170);
        assert_eq!(shell.measured, 40.0);
        assert_eq!(shell.sample, 0);
        assert_eq!(sleep, 40 - 10, "budget minus the 10ms dispatch");
    }

    /// The slow-window decay (both arms) floors `measured` at 5.0, and
    /// the menu branch folds `measured` back into `factor` on exit.
    #[test]
    fn fps_machine_decay_and_handoff() {
        let mut shell = Shell::new();
        let mut game = Game::new(vec![]);
        let mut xt = XtGraphics::new();
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases = crate::stage::loadbase().expect("loadbase");

        // 176 arm, slow window: 6.0 - 5.0 floors at 5.0
        xt.fase = 176;
        shell.loadhold = 50;
        shell.measured = 6.0;
        shell.sample = 10;
        shell.sample_at = 1000;
        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 1000, 2000);
        assert_eq!(shell.measured, 5.0);
        assert_eq!(shell.sample, 0);

        // plain menu arm: racing folds back (and zeroes the window),
        // then the 400ms windows run
        xt.fase = 10;
        shell.racing = true;
        shell.measured = 20.0;
        shell.factor = 35.0;
        shell.sample = 7;
        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 2000, 2000);
        assert!(!shell.racing);
        assert_eq!(shell.factor, 20.0, "measured folds into factor");
        assert_eq!(shell.sample, 1, "reset zeroes, then the window ticks");

        // loadhold == 1 takes factor over even mid-countdown
        xt.fase = 176;
        shell.loadhold = 1;
        shell.measured = 42.0;
        shell.factor = 35.0;
        shell.sample = 3;
        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 2000, 2000);
        assert_eq!(shell.factor, 42.0);
    }

    /// The race branch (GameSparker.java:1459-1485): arming copies
    /// `factor` into `measured`; a settled countdown eases the fade.
    #[test]
    fn fps_machine_race_handoff_and_fade() {
        let mut shell = Shell::new();
        let mut game = Game::new(vec![]);
        let mut xt = XtGraphics::new();
        xt.fase = 0;
        xt.starcnt = 0;
        let mut m = Medium::new();
        // lift the fade ceiling so the ease is observable (fresh media
        // sit at the 3000 floor, where any ease clamps back to 3000)
        m.origfade = 8000;
        m.fade[0] = 8000;
        let before = m.fade[0];
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases = crate::stage::loadbase().expect("loadbase");

        shell.factor = 35.0;
        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 5000, 5000);
        assert!(shell.racing);
        assert_eq!(shell.measured, 35.0);
        assert_eq!(shell.sample, 1);

        // ten quiet frames, then the window: 10 < 530ms eases the fade
        for i in 1..10 {
            shell.frame(
                &mut game,
                &bases,
                &mut xt,
                &mut m,
                &mut cv,
                5000 + i * 10,
                5000 + (i + 1) * 10,
            );
        }
        shell.measured = 14.0;
        shell.sample = 10;
        shell.sample_at = 5100;
        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 5100, 5150);
        assert_eq!(shell.measured, 14.5);
        assert_ne!(m.fade[0], before, "adjstfade ran at starcnt 0");
        assert_eq!(m.fade[0], 7500);
    }

    /// The sleep math (GameSparker.java:1488-1494): `round(measured) −
    /// elapsed`, floored at `min_sleep`.
    #[test]
    fn fps_sleep_clamps_at_min() {
        let mut shell = Shell::new();
        let mut game = Game::new(vec![]);
        let mut xt = XtGraphics::new();
        xt.fase = 10;
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases = crate::stage::loadbase().expect("loadbase");

        shell.measured = 35.0;
        shell.sample_at = 0;
        // elapsed past the whole budget: negative sleeps floor at 15
        let sleep = shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 0, 1000);
        assert_eq!(sleep, 15);
        // an exact-zero budget still floors: the source clamps everything
        // below min_sleep (GameSparker.java:1492-1494)
        shell.sample = 0;
        let sleep = shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 965, 1000);
        assert_eq!(sleep, 15);
        // headroom above the floor passes through untouched
        shell.sample = 0;
        let sleep = shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 990, 1000);
        assert_eq!(sleep, 25);
    }

    /// `exwist` stops every voice and reports through `exiting()`.
    #[test]
    fn exwist_stops_all_and_reports() {
        let mut shell = Shell::new();
        let mut game = Game::new(vec![]);
        let mut xt = XtGraphics::new();
        xt.fase = 10;
        xt.loadedt[0] = true;
        let raw: &[u8] = include_bytes!("../../../assets/music/stage1.zipo");
        let z = crate::zipo::Zipo::read(raw).unwrap();
        let mut r = crate::radicalmod::RadicalMod::new(&z.entries[0].data, 350, 8400, 135);
        r.play();
        xt.stracks[0] = Some(r);
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases = crate::stage::loadbase().expect("loadbase");

        assert!(!shell.exiting());
        shell.exwist = true;
        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 0, 0);
        assert!(shell.exiting());
        assert!(!xt.stracks[0].as_ref().unwrap().playing);
    }

    /// The fase-5 run-once (GameSparker.java:769-773): the first visit
    /// saves the car cookie; later visits don't rewrite.
    #[test]
    fn fase5_saves_the_car_once() {
        let (store, path) = cookie_store("fase5");
        let mut shell = Shell::new();
        shell.store = Some(store);
        let mut game = Game::new(vec![]);
        game.cp.stage = 1;
        let mut xt = XtGraphics::new();
        xt.fase = 5;
        xt.sc[0] = 7;
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases = crate::stage::loadbase().expect("loadbase");

        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 0, 0);
        assert!(shell.saved);
        // the frame mixed stage 1 (cp.stage defaults to 1)
        let store = shell.store.as_ref().unwrap();
        assert_eq!(store.read("usercar"), 7);
        let text = std::fs::read_to_string(&path).unwrap();

        // second visit: no rewrite (run-once held)
        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 100, 100);
        assert!(shell.saved);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        let _ = std::fs::remove_file(&path);
    }

    /// The -5 result saves (GameSparker.java:729-733): the newest-stage
    /// win bumps `unlocked`, then factor and car persist; `saved` resets.
    #[test]
    fn minus5_saves_unlocked_factor_car() {
        let (store, path) = cookie_store("minus5");
        let mut shell = Shell::new();
        shell.store = Some(store);
        shell.saved = true;
        shell.factor = 33.7;
        let mut game = Game::new(vec![]);
        let mut xt = XtGraphics::new();
        xt.loadpak();
        xt.fase = -5;
        xt.winner = true;
        xt.unlocked = 1;
        game.cp.stage = 1;
        xt.sc[0] = 4;
        let mut m = Medium::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let bases = crate::stage::loadbase().expect("loadbase");

        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 0, 0);
        assert!(!shell.saved);
        let store = shell.store.as_ref().unwrap();
        assert_eq!(store.read("unlocked"), 2);
        assert_eq!(store.read("gameprfact"), 33, "(int) truncation");
        assert_eq!(store.read("usercar"), 4);
        let _ = std::fs::remove_file(&path);
    }

    /// The race mouse press (GameSparker.java:1045-1049): `mouses == 1`
    /// in the race arm synthesizes enter for the next frame.
    #[test]
    fn race_mouse_press_synthesizes_enter() {
        let bases = crate::stage::loadbase().expect("loadbase");
        let mut m = Medium::new();
        let cars = build_cars(&bases, &m);
        let mut game = Game::new(cars);
        let mut shell = Shell::new();
        let mut xt = XtGraphics::new();
        xt.loadpak();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        let scene = crate::stage::loadstage(1, &bases, &mut m, &mut game.t, &mut game.cp, &mut xt)
            .expect("stage 1 loads");
        game.scene = Some(scene);
        xt.fase = 0;
        shell.mouses = 1;
        shell.frame(&mut game, &bases, &mut xt, &mut m, &mut cv, 0, 16);
        assert!(game.ctl[0].enter);
        assert_eq!(shell.mouses, 0);
    }
}
