//! Milestone renderer: two modes.
//!
//! Play mode is the default: no arguments boots the applet ctor, cookies and
//! the real-time minifb window (see below). The headless modes each start
//! with an explicit keyword.
//!
//! Car mode (`car <zipo> <car> <out.bmp> [yaw]`, also the fallback for an
//! unrecognized first argument): parses one `.rad` out of `models.zipo`,
//! builds the runtime model (planes + `Wheels.make` wheel planes), places it,
//! frames it with the ported chase camera and paints one 550x400 frame into a
//! BMP.
//!
//! ```sh
//! cargo run -p nfm-render
//! cargo run -p nfm-render -- assets/graphics/models.zipo king render/king.bmp 90
//! ```
//!
//! Stage mode (`stage <n>`): parses `assets/stages/<n>.txt` with the
//! faithful `loadstage()` port (palette + placements in file order),
//! builds the pool with the 5 car start slots, and paints the stage-load
//! flyover frame (`Medium.aroundtrack`) the way the applet's stage-select
//! view rasterizes it: pool[5..notb] — dist==0 contours drawn in slot
//! order first, the rest via the count sort (ties favor the earlier pool
//! slot, GameSparker.java:830-852).
//!
//! ```sh
//! cargo run -p nfm-render -- stage 1
//! cargo run -p nfm-render -- stage 1 render/stage1f25.bmp 25
//! ```
//!
//! The optional 3rd arg runs that many full `fase == 1` runs first — each
//! is `nfm_core::gamesparker::fase1` (GameSparker.java:809-852: backdrop,
//! `aroundtrack` flyover camera step, pool draw with the count sort) — so
//! the capture can descend the flyover until contour dists are live and
//! the count sort engages. The final capture is run number frames+1: the
//! camera steps and draws pair exactly as in the applet.
//!
//! Note: on the first runs the contour-camera int sum (dy ~ -60000)
//! overflow-wraps exactly as in the applet (ContO.java:618-622), and
//! sqrt(NaN) -> (int)0 sets dist 0, so the draw rasterizes everything in
//! pool order — parity by wrap-around.
//!
//! GIF/JPEG mode (`gif <zipo> <entry> <out.bmp>` / `jpeg <zipo> <entry>
//! <out.bmp>`): decode one sprite entry and save it as a BMP (gif
//! composites the keyed-out pixels over magenta).
//!
//! ```sh
//! cargo run -p nfm-render -- gif assets/graphics/images4.zipo game.gif
//! cargo run -p nfm-render -- jpeg assets/graphics/images2.zipo bgmain.jpg
//! ```
//!
//! Not drawn here (UI-only, pending): the xt screens that consume the
//! decoded `bgmain`/`trackbg` sprites (the fase 4/5 menu backdrop, the
//! stage-select `trackbg` view) are later xt slices.
//!
//! Menu mode (`menu <stage> [frames] [out]`): the canonical applet walk
//! through `Shell::frame` (clicknow skip, rad splash, New Game, inst ×3,
//! carselect, stage load, stageselect, loadmusic, the 176 countdown,
//! musicomp, then N race frames), dumping the final frame with a one-line
//! fase trace.
//!
//! Play mode (`play [stage]`, and the no-argument default): the same boot the
//! applet ctor runs —
//! loadbase, the cookie jar, the sprite/music spine, the carselect
//! roster — then the real-time loop against a minifb window
//! (`window::Window`): frames dispatch through `Shell::frame`, sleeps
//! pace the loop, and closing the window is the applet stop (the loop
//! flips `exwist` and runs one last frame so `stopallnow` silences every
//! voice). Prints a one-line session summary on exit.
//!
//! Playing on a pad: the gilrs adapter (`gamepad::Gamepad`) is handed to
//! the window, which pumps it once per frame through
//! `RealTimeHost::pump` — its edges queue with the keyboard's —
//! RT/X/LT drive, the stick and D-pad steer, A is confirm-or-arrow-aim by
//! screen and Start is always enter. No other mode touches it.
//!
//! Debug builds add the Deck's self-diagnosis layer, compiled out of
//! release binaries entirely: `debugserve` (a read-only LAN server on port
//! 8085 for the run log, the latest frame, the WATCH/edge state, the
//! cookie jar, the probe log and an env allow-list), `logger` (the play
//! lines also land in `nfm.log`) and `probe` (a one-time 120s in-window pad
//! probe gated on `nfm-probe.done` beside the executable). The probe runs
//! before the realtime loop; skip it by keeping that marker file, and delete
//! it to probe again.

use std::process::ExitCode;

use nfm_core::canvas::Canvas;
use nfm_core::checkpoints::CheckPoints;
use nfm_core::shell::{Game, Shell};

mod audio;
mod gamepad;
mod logger;
mod window;

#[cfg(debug_assertions)]
mod debugserve;
#[cfg(debug_assertions)]
mod probe;

use nfm_core::conto::{Base, Conto, follow};
use nfm_core::control::Control;
use nfm_core::gamesparker;
use nfm_core::gif::Gif;
use nfm_core::madness::Madness;
use nfm_core::medium::Medium;
use nfm_core::model::Model;
use nfm_core::record::Record;
use nfm_core::stage::Scene;
use nfm_core::trackers::Trackers;
use nfm_core::wheels::Wheels;
use nfm_core::xtgraphics::XtGraphics;
use nfm_core::zipo::Zipo;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("stage") {
        stage_mode(&args[1..])
    } else if args.first().map(String::as_str) == Some("race") {
        race_mode(&args[1..])
    } else if args.first().map(String::as_str) == Some("menu") {
        menu_mode(&args[1..])
    } else if args.first().map(String::as_str) == Some("play") {
        play_mode(&args[1..])
    } else if args.first().map(String::as_str) == Some("gif") {
        gif_mode(&args[1..])
    } else if args.first().map(String::as_str) == Some("jpeg") {
        jpeg_mode(&args[1..])
    } else if args.first().map(String::as_str) == Some("loadsnap") {
        loadsnap_mode(&args[1..])
    } else if args.first().map(String::as_str) == Some("pressed") {
        pressed_mode(&args[1..])
    } else if args.is_empty() {
        // No arguments = the shipped-single-binary entry point: play mode
        // with its own defaults (`play` == `play 1`: stage 1 from the boot
        // menu). Every other mode keeps its explicit keyword.
        play_mode(&[])
    } else {
        car_mode(&args)
    }
}

/// Decode one GIF entry from an archive and save it as a BMP, transparent
/// pixels composited over magenta so the sprite's shape and the keyed
/// regions both show: `gif <zipo> <entry> <out.bmp>`.
fn gif_mode(args: &[String]) -> ExitCode {
    let zipo_path = args
        .first()
        .map(String::as_str)
        .unwrap_or("assets/graphics/images1.zipo");
    let name = args.get(1).map(String::as_str).unwrap_or("lap.gif");
    let out = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("render/giffed.bmp");

    let raw = match std::fs::read(zipo_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {zipo_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let zipo = match Zipo::read(&raw) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("bad zipo: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(entry) = zipo.get(name) else {
        eprintln!("no {name} in archive");
        return ExitCode::FAILURE;
    };
    let gif = match nfm_core::gif::decode(&entry.data) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{name}: {e}");
            return ExitCode::FAILURE;
        }
    };
    save_composited(&gif, name, out)
}

/// Decode one JPEG entry and save it as a BMP — the toolkit-path eyeball
/// for the two image-wide sprites: `jpeg <zipo> <entry> <out.bmp>`.
fn jpeg_mode(args: &[String]) -> ExitCode {
    let zipo_path = args
        .first()
        .map(String::as_str)
        .unwrap_or("assets/graphics/images2.zipo");
    let name = args.get(1).map(String::as_str).unwrap_or("bgmain.jpg");
    let out = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("render/jpeged.bmp");

    let raw = match std::fs::read(zipo_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {zipo_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let zipo = match Zipo::read(&raw) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("bad zipo: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(entry) = zipo.get(name) else {
        eprintln!("no {name} in archive");
        return ExitCode::FAILURE;
    };
    let gif = match nfm_core::jpeg::decode(&entry.data) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{name}: {e}");
            return ExitCode::FAILURE;
        }
    };
    save_composited(&gif, name, out)
}

/// Save a sprite as a BMP, transparent pixels composited over magenta so
/// the sprite's shape and the keyed regions both show. Shared tail of the
/// sprite modes.
fn save_composited(g: &Gif, name: &str, out: &str) -> ExitCode {
    let mut cv = Canvas::new(g.w, g.h);
    let mut transparent = 0usize;
    for (i, &a) in g.px.iter().enumerate() {
        let t = &mut cv.px[i * 3..i * 3 + 3];
        if a >> 24 == 0 {
            transparent += 1;
            t[0] = 255;
            t[1] = 0;
            t[2] = 255;
        } else {
            t[0] = (a >> 16) as u8;
            t[1] = (a >> 8) as u8;
            t[2] = a as u8;
        }
    }
    match cv.save_bmp(out) {
        Ok(_) => {
            println!(
                "{name}: {w}x{h}, {op} opaque px, out {out}",
                w = g.w,
                h = g.h,
                op = g.px.len() - transparent
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cannot write {out}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Read one entry out of a zipo archive and decode it as a GIF.
fn decode_entry(zipo_path: &str, name: &str) -> Result<Gif, String> {
    let raw = std::fs::read(zipo_path).map_err(|e| format!("cannot read {zipo_path}: {e}"))?;
    let zipo = Zipo::read(&raw).map_err(|e| format!("bad zipo: {e}"))?;
    let entry = zipo
        .get(name)
        .ok_or_else(|| format!("no {name} in archive"))?;
    nfm_core::gif::decode(&entry.data).map_err(|e| format!("{name}: {e}"))
}

/// `loadsnap <zipo> <entry> <sr> <sg> <sb> <kr> <kg> <kb> <out.bmp>` — run
/// `xtGraphics.loadsnap` with the given stage `snap` palette and sky color
/// (the sky as `Medium.setsky` stores it: already snap-blended, clamped).
/// Defaults are stage 1's palette.
fn loadsnap_mode(args: &[String]) -> ExitCode {
    let zipo_path = args
        .first()
        .map(String::as_str)
        .unwrap_or("assets/graphics/images1.zipo");
    let name = args.get(1).map(String::as_str).unwrap_or("lap.gif");
    let int =
        |i: usize, dflt: i32| -> i32 { args.get(i).and_then(|s| s.parse().ok()).unwrap_or(dflt) };
    let out = args
        .get(8)
        .map(String::as_str)
        .unwrap_or("render/loadsnap.bmp");

    let g = match decode_entry(zipo_path, name) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let mut m = Medium::new();
    m.snap = [int(2, -5), int(3, -5), int(4, 20)];
    m.csky = [int(5, 190), int(6, 215), int(7, 255)];
    save_composited(&nfm_core::xtgraphics::loadsnap(&g, &m), name, out)
}

/// `pressed <zipo> <entry> <out.bmp>` — run `xtGraphics.pressed` (the
/// pressed button silhouette).
fn pressed_mode(args: &[String]) -> ExitCode {
    let zipo_path = args
        .first()
        .map(String::as_str)
        .unwrap_or("assets/graphics/images1.zipo");
    let name = args.get(1).map(String::as_str).unwrap_or("next.gif");
    let out = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("render/pressed.bmp");

    let g = match decode_entry(zipo_path, name) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    save_composited(&nfm_core::xtgraphics::pressed(&g), name, out)
}

/// Shared stage boot: `loadbase()` over models.zipo, then the stage load
/// (the `stages/<n>.txt` parse and the pool build) — the common spine of
/// the `stage`, `race` and `menu` modes. Returns the built scene with its
/// working `Medium`/`Trackers`/`CheckPoints` plus the xt screen state
/// (the applet's xt spans stage loads; this per-load stand-in runs the
/// same archive/reset spine) and the stage name.
fn stage_setup(
    stage_n: i32,
) -> Result<(Scene, Medium, Trackers, CheckPoints, XtGraphics, String), String> {
    // models always come from the repo's models.zipo like the applet's
    // loadbase()
    let bases = nfm_core::stage::loadbase()?;

    let mut m = Medium::new();
    let mut t = Trackers::new();
    let mut cp = CheckPoints::new();
    // xtGraphics init (xt:2905-2987): the archive spine runs once at
    // applet init; this per-load stand-in loadpaks, and loadstage runs
    // the HUD reset ahead of the parse (see stage::loadstage).
    let mut xt = XtGraphics::new();
    xt.loadpak();
    let scene = nfm_core::stage::loadstage(stage_n, &bases, &mut m, &mut t, &mut cp, &mut xt)?;
    let name = cp.name.clone();
    Ok((scene, m, t, cp, xt, name))
}

fn stage_mode(args: &[String]) -> ExitCode {
    // `stage <n> [out] [frames]`
    let stage_n: i32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    let out_owned;
    let out = match args.get(1) {
        Some(s) => s.as_str(),
        None => {
            out_owned = format!("render/stage{stage_n}.bmp");
            out_owned.as_str()
        }
    };

    let (mut scene, mut m, t, cp, _xt, name) = match stage_setup(stage_n) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    // Each stage-view frame is one full run of the applet's fase==1 block
    // (nfm_core::gamesparker::fase1 — GameSparker.java:809-852: backdrop,
    // aroundtrack flyover camera step, pool draw with the count sort).
    // The camera descends 3000 units per run, so a few runs in, contour
    // dists are real and the final capture exercises the count sort
    // instead of the wrapping-collapsed pool-order path.
    let frames: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    for _f in 0..frames {
        let mut prime = Canvas::new(m.w as usize, m.h as usize);
        gamesparker::fase1(&mut scene, &mut prime, &mut m, &t, &cp);
    }

    // final stage-view run — this run IS what paints the output
    let mut cv = Canvas::new(m.w as usize, m.h as usize);
    let n = gamesparker::fase1(&mut scene, &mut cv, &mut m, &t, &cp);

    if let Some(dir) = std::path::Path::new(out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = cv.save_bmp(out) {
        eprintln!("cannot write {out}: {e}");
        return ExitCode::FAILURE;
    }
    let painted = cv.px.chunks_exact(3).filter(|p| *p != [0u8, 0, 0]).count();
    println!(
        "stage {stage_n} `{name}`: {} contours in pool, notb {}, wall trackers {}, {} sorted + rest direct, center ({}, {}), {painted} painted px, out {out}",
        scene.pool.len(),
        scene.notb,
        t.nt,
        n,
        scene.trx,
        scene.trz
    );
    ExitCode::SUCCESS
}

fn race_mode(args: &[String]) -> ExitCode {
    // `race <n> [out] [frames] [starcnt] [view] [press] [replays]` — the
    // fase==0 racing frame skeleton (nfm_core::gamesparker::fase0): view
    // camera, the race draw over pool[0..nob] with the five car slots
    // included, then the countdown tick. The physics chain incl. the AI
    // `preform` runs under fase0 per frame; the starcnt==90-orbit intro
    // and the xt `stat` HUD are the next racing slices. The scripted
    // press string stands in for the keyboard event path on slot 0 (the
    // xt milestone); slots 1..4 press for themselves through `preform`.
    // `replays` (arg 7, default 0) hands the finished race to the
    // fase==-1 reply film — the capture then re-paints the film's last
    // frame, and `h`/enter presses skip the film to its end.
    // `crash` (arg 8, default 0) runs the fase==-2/-3 wasted crash-cam
    // chain instead of the reply: the film plays off `hcaught` and the
    // capture keeps its last painted frame.
    let stage_n: i32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    let out_owned;
    let out = match args.get(1) {
        Some(s) => s.as_str(),
        None => {
            out_owned = format!("render/stage{stage_n}race.bmp");
            out_owned.as_str()
        }
    };
    let frames: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    // GameSparker.java:1058 — the `starcnt == 35` transition rides the
    // fase0 lanes now; the default 34 starts below it on the steady
    // follow view, decrementing each run like GameSparker.java:1025-1027
    let mut starcnt: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(34);
    // fase lives on xt (the source's var6.fase): the stat calls in fase0
    // transition it (the -2/-6 result screens; the capture loop keeps
    // running its frames regardless, like the harness it is).
    // 0 = follow(car0), 1 = around(car0,false), 2 = watch(car0)
    let view: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
    // Constant scripting stand-in for the keyboard event path (the
    // xt/Event milestone): letters u/l/d/r/h = up/left/down/right/conf,
    // e.g. `render ... u` holds throttle for the whole run. The AI
    // competitor slots press through their own `preform` call.
    let press = args.get(5).cloned().unwrap_or_default();
    let up = press.contains('u');
    let down = press.contains('d');
    let left = press.contains('l');
    let right = press.contains('r');
    let handb = press.contains('h');

    // 6 = replay film runs after the race (the fase == -1 pass set; the
    // capture re-paints the film's last frame), default 0. The skip keys
    // (h in the press string) jump the film to its 299 end frame.
    let replays: usize = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
    // 7 = the wasted crash-cam chain after the race (the fase == -2/-3
    // pass set), default 0: fase -2 arms the film off `hcaught` and
    // fase -3 plays it — the capture keeps the film's last painted
    // frame, or its next one while the film is still rolling.
    let crash: usize = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);

    let (mut scene, mut m, t, mut cp, mut xt, name) = match stage_setup(stage_n) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    // loadstage tail (GameSparker.java:560-572): all five Controls reset
    // against the checkpoints, then each car slot is reset through
    // Madness.reseto. The `sc` car numbers drive the per-car stat tables —
    // sc[0] is the player's pick, and xt's sortcars has dealt the
    // opponents during stage_setup (the menu slice feeds sc[0]).
    let mut mad = [
        Madness::new(0),
        Madness::new(1),
        Madness::new(2),
        Madness::new(3),
        Madness::new(4),
    ];
    let mut u = [
        Control::new(),
        Control::new(),
        Control::new(),
        Control::new(),
        Control::new(),
    ];
    for e in u.iter_mut() {
        e.reset(&cp);
    }
    for (i, (md, o)) in mad.iter_mut().zip(scene.pool.iter_mut()).enumerate() {
        md.reseto(xt.sc[i] as usize, o, &mut cp);
    }
    // GameSparker.java:574 — the loadstage-tail recorder priming.
    let mut rpd = Record::new();
    rpd.reset(&scene.pool, &m);

    // The var8 stand-in (GameSparker.java:927-940 — the fase-0 rebuild
    // source is `var8[cn]`, the loadbase model entry): one fresh baseline
    // per roster model, cloned from its pool car slot — the pool cars are
    // pristine placements at this point, geometrically the model entries.
    // (Indexing the pool by model id here would clone a different car's
    // live contour — or a track piece — once the roster is dealt.)
    let mut newcar: Vec<Option<Conto>> = (0..43).map(|_| None).collect();
    for i in 0..5 {
        newcar[xt.sc[i] as usize] = Some(Conto::clone_from(&scene.pool[i], &m, 0, 0, 0, 0));
    }

    // The last run's canvas is the saved capture: one fase0 run per
    // history frame throws its painting away exactly like the stage mode,
    // and the final run paints what gets written out. With a film count
    // the race's last frame hands to the replay instead (fase == -1), so
    // the film passes run first and the capture re-paints the film's
    // last frame.
    // The race's history frames each throw their painting away; the
    // last run's canvas is the saved capture. With a film count the
    // race's last frame hands to the replay instead (fase == -1), so
    // the film passes run first and the capture re-paints the film's
    // last frame.
    // The applet paints every frame into ONE persistent offscreen image;
    // the flex==2 partial HUD redraw (xt:1744's delta counters and the
    // medium's band-only backdrop) assumes the previous frame's pixels
    // are still there, so the capture reuses the canvas across runs.
    let mut cv = Canvas::new(m.w as usize, m.h as usize);
    for _ in 0..frames {
        u[0].up = up;
        u[0].down = down;
        u[0].left = left;
        u[0].right = right;
        u[0].handb = handb;
        let _ = gamesparker::fase0(
            &mut scene,
            &mut mad,
            &mut u,
            &mut cp,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut starcnt,
            view,
            &newcar,
            &mut xt,
        );
        // GameSparker.java:1405 — the audio hub's state tail runs every
        // loop iteration after the fase dispatch.
        xt.playsounds(&mad[0], &u[0], cp.stage, &mut m);
    }
    // the capture keeps painting on the SAME canvas the runs primed (the
    // flex==2 delta redraw assumes the previous frame's pixels).
    let n;
    if crash > 0 {
        // fase == -2/-3: the wasted crash-cam chain. The post-race xt
        // screen's enter hands fase to -2 (xtGraphics.java:1607); the
        // arm seeds the film off the recorded wasted event and each
        // pass plays a film frame. Discard passes walk the film; the
        // capture keeps the last painted frame when the film has closed
        // out to fase == -4 (the double sweep or a skip press), and
        // paints one more film frame otherwise.
        let (cw, ch) = (m.w as usize, m.h as usize);
        xt.fase = -2;
        let (mut creplay, mut chold, mut csweeps) = (0i32, 0i32, 0i32);
        gamesparker::fasewasted2(
            &mut m,
            &mut cv,
            &rpd,
            &mut creplay,
            &mut chold,
            &mut csweeps,
            &mut xt,
        );
        let mut last_n = 0usize;
        let mut last: Option<Canvas> = None;
        for _ in 0..crash {
            if xt.fase != -3 {
                break;
            }
            let mut film_cv = Canvas::new(cw, ch);
            last_n = gamesparker::fasewasted3(
                &mut scene,
                &mut mad,
                &mut u,
                &mut film_cv,
                &mut m,
                &t,
                &mut rpd,
                &mut creplay,
                &mut chold,
                &mut csweeps,
                &mut xt,
                &newcar,
            );
            last = Some(film_cv);
        }
        if xt.fase == -3 {
            let mut film_cv = Canvas::new(cw, ch);
            n = gamesparker::fasewasted3(
                &mut scene,
                &mut mad,
                &mut u,
                &mut film_cv,
                &mut m,
                &t,
                &mut rpd,
                &mut creplay,
                &mut chold,
                &mut csweeps,
                &mut xt,
                &newcar,
            );
            cv = film_cv;
        } else if let Some(last_cv) = last {
            cv = last_cv;
            n = last_n;
        } else {
            // fase == -4 without a wasted event: the trackbg backdrop the
            // handoff blit onto the capture stays as the frame.
            n = 0;
        }
    } else if replays > 0 {
        // fase == -1: the race's last frame hands the pool to the replay
        // film — `var22` seeds ocar at frame 0, each discard pass walks
        // a film frame, and the capture paints the film's last frame.
        let mut replay = 0i32;
        let (cw, ch) = (m.w as usize, m.h as usize);
        let mut film = |cv: &mut Canvas| -> usize {
            u[0].up = up;
            u[0].down = down;
            u[0].left = left;
            u[0].right = right;
            u[0].handb = handb;
            gamesparker::fasereplay1(
                &mut scene,
                &mut mad,
                &mut u,
                cv,
                &mut m,
                &t,
                &mut rpd,
                &mut replay,
                &newcar,
            )
        };
        for _ in 0..replays {
            let mut film_cv = Canvas::new(cw, ch);
            film(&mut film_cv);
        }
        n = film(&mut cv);
    } else {
        u[0].up = up;
        u[0].down = down;
        u[0].left = left;
        u[0].right = right;
        u[0].handb = handb;
        n = gamesparker::fase0(
            &mut scene,
            &mut mad,
            &mut u,
            &mut cp,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut starcnt,
            view,
            &newcar,
            &mut xt,
        );
        // GameSparker.java:1405 — the audio hub's state tail runs every
        // loop iteration after the fase dispatch.
        xt.playsounds(&mad[0], &u[0], cp.stage, &mut m);
    }
    let painted = cv.px.chunks_exact(3).filter(|p| *p != [0u8, 0, 0]).count();
    if let Err(e) = cv.save_bmp(out) {
        eprintln!("cannot write {out}: {e}");
        return ExitCode::FAILURE;
    }
    println!(
        "race {stage_n} `{name}`: {} contours in pool, notb {}, nob {}, wall trackers {}, {frames} runs, {replays} film, {crash} crash-cam, starcnt -> {starcnt}, view {view}, {} sorted + rest direct, {painted} painted px, out {out} | car0 speed {:.2} wxz {} wzy {} xz {} y {}",
        scene.pool.len(),
        scene.notb,
        scene.nob,
        t.nt,
        n,
        mad[0].speed,
        scene.pool[0].wxz,
        scene.pool[0].wzy,
        scene.pool[0].xz,
        scene.pool[0].y,
    );
    if crash > 0 {
        println!(
            "crash-cam: hcaught {} wasted {} when {}",
            rpd.hcaught, rpd.wasted, rpd.whenwasted
        );
    }
    ExitCode::SUCCESS
}

fn menu_mode(args: &[String]) -> ExitCode {
    // `menu <stage> [frames] [out]` — the canonical applet walk through
    // `Shell::frame` (the same script the shell walk test drives: skip
    // clicknow with a press, rad ×200, New Game, inst ×3, carselect enter,
    // stage load, stageselect enter, loadmusic, the 176 countdown,
    // musicomp enter, then N race frames). Dumps the final frame and
    // prints a one-line fase trace.
    let stage_n: i32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    let frames: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let out_owned;
    let out = match args.get(2) {
        Some(s) => s.as_str(),
        None => {
            out_owned = format!("render/menu{stage_n}.bmp");
            out_owned.as_str()
        }
    };

    let bases = match nfm_core::stage::loadbase() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let mut m = Medium::new();
    let cars = nfm_core::shell::build_cars(&bases, &m);
    let mut game = nfm_core::shell::Game::new(cars);
    game.cp.stage = stage_n;
    let mut shell = nfm_core::shell::Shell::new();
    let mut xt = XtGraphics::new();
    xt.loadpak();
    let mut cv = Canvas::new(m.w as usize, m.h as usize);
    let mut trace = vec![xt.fase];

    // Real wall-clock stamps for the fps machine (the sleep it returns
    // is discarded — captures don't throttle).
    let wall_ms = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    };
    macro_rules! step {
        () => {{
            let start = wall_ms();
            let _sleep = shell.frame(
                &mut game,
                &bases,
                &mut xt,
                &mut m,
                &mut cv,
                start,
                wall_ms(),
            );
            trace.push(xt.fase);
        }};
    }

    step!(); // clicknow
    shell.mouses = 1;
    step!(); // skip into the splash
    for _ in 0..200 {
        step!(); // rad ×200 into the main menu
    }
    game.ctl[0].up = true;
    step!(); // opselect 1 → 0 (New Game)
    game.ctl[0].enter = true;
    step!(); // maini → instructions
    for _ in 0..3 {
        game.ctl[0].enter = true;
        step!(); // inst ×3 → car select
    }
    game.ctl[0].enter = true;
    step!(); // carselect enter → stage load → stage select
    game.ctl[0].enter = true;
    step!(); // stageselect enter → music load
    step!(); // loadmusic → 176 countdown
    let mut guard = 0;
    while xt.fase == 176 && guard < 1000 {
        step!();
        guard += 1;
    }
    game.ctl[0].enter = true;
    step!(); // musicomp enter → first race frame
    for _ in 1..frames {
        step!();
    }

    if let Some(dir) = std::path::Path::new(out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = cv.save_bmp(out) {
        eprintln!("cannot write {out}: {e}");
        return ExitCode::FAILURE;
    }
    trace.dedup();
    let ink = cv.px.chunks_exact(3).filter(|p| *p != [0u8, 0, 0]).count();
    println!(
        "menu {stage_n}: fase trace {trace:?}, {frames} race frames, {ink} painted px, out {out}"
    );
    ExitCode::SUCCESS
}

/// Play mode (`play [stage]`): boot exactly like the applet ctor, then
/// the real-time loop against a minifb window. A fresh mix bus is bound
/// to the shell and a cpal device opened on it (8000 Hz mono i16 when
/// offered, else the closest the device allows; failures print and play
/// runs silent). Closing the window is the applet stop: the loop flips
/// `exwist` and one last frame runs so `stopallnow()` silences every
/// voice (GameSparker.java:1478-1486).
fn play_mode(args: &[String]) -> ExitCode {
    let stage_n: i32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(1);

    // The constructor spine: loadbase, then the sprite/music load.
    let bases = match nfm_core::stage::loadbase() {
        Ok(b) => b,
        Err(e) => {
            logger::error(&e);
            return ExitCode::FAILURE;
        }
    };
    let store = nfm_core::cookies::CookieStore::default_path();
    logger::line(&format!("cookies: {}", store.path().display()));
    let mut m = Medium::new();
    let mut shell = Shell::new();
    let mut xt = XtGraphics::new();
    xt.loadpak();
    let cars = nfm_core::shell::build_cars(&bases, &m);
    let mut game = Game::new(cars);
    // Ctor defaults first, then the cookie override — the source's
    // order (GameSparker.java:610-633 runs after the defaults).
    game.cp.stage = stage_n;
    shell.read_cookies(&mut xt, &mut game.cp, &store);
    // Hand the jar to the shell: every save site is gated on `store`
    // (fase −5 at shell.rs:296-304, fase 5 at :340), so leaving it None
    // silently swallows the whole persistence protocol — the source's
    // var37 latch saves (GameSparker.java:729-733 and :774). Probed by
    // T1's cookie-latch scenario in both suites; the end-to-end smoke is a
    // real play session.
    shell.store = Some(store);
    let mut cv = Canvas::new(m.w as usize, m.h as usize);

    let mut win = match window::Window::new("Need for Madness") {
        Ok(w) => w,
        Err(e) => {
            logger::error(&format!("cannot open window: {e}"));
            return ExitCode::FAILURE;
        }
    };

    // Debug builds: the read-only LAN evidence server starts right after
    // the window, so the probe below is already observable (`/frame.bmp`,
    // `/keys`). A taken port is one log line; play never blocks on it.
    #[cfg(debug_assertions)]
    let debug_server = debugserve::DebugServer::start(win.debug_share());

    // Debug builds: the one-time in-window pad probe (skipped once
    // `nfm-probe.done` sits beside the executable). It runs on this window
    // through the same present path the game loop uses, before the play
    // gamepad adapter opens its own gilrs instance.
    #[cfg(debug_assertions)]
    probe::run(&mut win);

    // The gamepad adapter exists on the play path only (headless captures
    // and the mirror suites never construct it): a gilrs failure is one log
    // line and keyboard-only play, never a panic. The window owns it and
    // pumps it per frame via RealTimeHost::pump.
    let pad = gamepad::Gamepad::open();
    #[cfg(debug_assertions)]
    if let Some(pad) = &pad {
        win.debug_pads(pad.names());
    }
    win.set_gamepad(pad);

    // The mix bus lives in Shell (bound to xt's voices on the first
    // audio-driven frame); the device backend drains the other end.
    // A failed open keeps audio None — play runs silent, headless-style.
    let bus = std::sync::Arc::new(std::sync::Mutex::new(nfm_core::audiobus::AudioBus::new()));
    shell.bind_audio(std::sync::Arc::clone(&bus));
    match audio::CpalSink::open(&bus) {
        Ok(sink) => {
            let summary = sink.describe();
            logger::line(&format!("audio: {summary}"));
            #[cfg(debug_assertions)]
            win.debug_audio(format!("{} — {summary}", audio::default_output_name()));
            win.set_audio(sink);
        }
        Err(e) => logger::error(&format!("audio unavailable ({e}); continuing silent")),
    }

    // The realtime loop is Shell's own (host.rs run_realtime); the window
    // pumps the pad through the host hook. Closing the window flips exwist
    // and runs one final frame so stopallnow() silences every voice.
    shell.run_realtime(&mut game, &bases, &mut xt, &mut m, &mut cv, &mut win);
    logger::line(&format!(
        "play: {} frames presented, exiting",
        win.presented()
    ));
    #[cfg(debug_assertions)]
    drop(debug_server);
    ExitCode::SUCCESS
}

fn car_mode(args: &[String]) -> ExitCode {
    let zipo_path = args
        .first()
        .map(String::as_str)
        .unwrap_or("assets/graphics/models.zipo");
    let name = args.get(1).map(String::as_str).unwrap_or("formula7");
    let out = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("render/formula7.bmp");
    let yaw: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);

    let raw = match std::fs::read(zipo_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {zipo_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let zipo = match Zipo::read(&raw) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("bad zipo: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(entry) = zipo.get(&format!("{name}.rad")) else {
        eprintln!("no {name}.rad in archive");
        return ExitCode::FAILURE;
    };
    let model = match Model::parse_rad(&entry.name, &entry.data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let mut m = Medium::new();
    let mut wheels = Wheels::new();
    let mut trackers = Trackers::new();

    // Base planes: <p> polys + 9 planes per wheel at their original slots
    let base = Base::new(&model, &m, &mut wheels);

    // place at origin, rotated `yaw` degrees
    let mut conto = Conto::place(&base, &m, &mut trackers, 0, 0, 0, yaw);

    // chase camera framing (Medium.follow): 800 units behind the car
    follow(&mut m, &conto, 0);

    // prime pass — a running game would have called these every frame
    let mut prime = Canvas::new(m.w as usize, m.h as usize);
    m.d(&mut prime);
    conto.d(&mut prime, &mut m, &trackers);

    // final frame
    let mut cv = Canvas::new(m.w as usize, m.h as usize);
    m.d(&mut cv);
    conto.d(&mut cv, &mut m, &trackers);

    if let Some(dir) = std::path::Path::new(out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = cv.save_bmp(out) {
        eprintln!("cannot write {out}: {e}");
        return ExitCode::FAILURE;
    }

    let painted = cv.px.chunks_exact(3).filter(|p| *p != [0u8, 0, 0]).count();
    println!(
        "{name}: {} planes ({} polys + {} wheels), grat {}, out {out} ({painted} painted px)",
        base.planes.len(),
        model.planes.len(),
        model.wheels.len(),
        base.grat
    );
    ExitCode::SUCCESS
}
