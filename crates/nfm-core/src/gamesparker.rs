// FIDELITY: GameSparker.java frame logic: indexed loops and the multi-argument Java method signatures are kept verbatim.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]
//! Port of the `GameSparker` applet-main-class frame logic. The original
//! class is the whole game shell; the pieces ported so far live here:
//! the `fase == 1` stage-view run (GameSparker.java:809-852), the
//! `fase == 0` racing run skeleton (GameSparker.java:926-1070) — both
//! share the pool draw with the painter's count sort below — the race
//! chain (colide/drive/rec/checkstat/preform, :985-1015), the race-end
//! reply film (`fase == -1`, :1072-1167, `fasereplay1`) and the wasted
//! crash-cam pair (`fase == -2`/`-3`, :1169-1316, `fasewasted2`/
//! `fasewasted3`). The `fase == -4` result screen lives on the xt side;
//! the `starcnt` orbit-camera countdown (36..90) plus the xt `stat` HUD
//! remain milestones.

use crate::canvas::Canvas;
use crate::checkpoints::{CheckPoints, PLAYERS};
use crate::conto::Conto;
use crate::control::Control;
use crate::madness::Madness;
use crate::medium::Medium;
use crate::record::Record;
use crate::stage::Scene;
use crate::trackers::Trackers;
use crate::xtgraphics::XtGraphics;

/// The shared pool-draw skeleton (the identical shape appears at
/// GameSparker.java:944-983 for the race view and 812-852 for the stage
/// view): every slot `lo..hi` whose `dist == 0` paints in slot order, and
/// the rest enter a count sort (rank = how many of the other slots are
/// farther) and paint far-to-near by rank.
///
/// Note the tie rule (GameSparker.java:838-842 / 969-973): in this loop
/// shape the inner index is always greater, so the `else if` is dead code
/// and ties always raise the EARLIER slot's rank — the earlier slot paints
/// later, on top. The plane-level tie inside `Conto::d` (ContO.java:
/// 586-600) resolves the opposite way.
///
/// Returns the number of pieces that went through the sort (`var27`/
/// `var42` in the original) — caller-side render telemetry only.
///
/// `pub(crate)` so the `fase == 176` shell arm (`shell::Shell::frame`)
/// can reuse it: the source's 176 draw is the same loop over
/// `pool[0..nob)`.
pub(crate) fn draw_sorted(
    scene: &mut Scene,
    cv: &mut Canvas,
    m: &mut Medium,
    t: &Trackers,
    lo: usize,
    hi: usize,
) -> usize {
    // GameSparker.java:946-953 — direct pieces and the sort roster
    let hi = hi.min(scene.pool.len());
    let mut order: Vec<usize> = Vec::new();
    for i in lo..hi {
        if scene.pool[i].dist == 0 {
            scene.pool[i].d(cv, m, t);
        } else {
            order.push(i);
        }
    }

    // GameSparker.java:961-975 — rank each slot by how many others are
    // farther; ties raise the earlier slot (see the doc note)
    let n = order.len();
    let mut cnt = vec![0usize; n];
    for a in 0..n {
        for b in (a + 1)..n {
            let da = scene.pool[order[a]].dist;
            let db = scene.pool[order[b]].dist;
            if da != db {
                if da < db {
                    cnt[a] += 1;
                } else {
                    cnt[b] += 1;
                }
            } else {
                cnt[a] += 1;
            }
        }
    }

    // GameSparker.java:977-983 — far-to-near paint by rank
    for slot in 0..n {
        for i in 0..n {
            if cnt[i] == slot {
                scene.pool[order[i]].d(cv, m, t);
            }
        }
    }

    n
}

/// One `fase == 1` run of the applet (GameSparker.java:810-852): the
/// flyover camera step, the backdrop fill, then the pool draw over
/// `[5..notb)`.
///
/// Backdrop note: the original lands here with `trackbg` first
/// (GameSparker.java:810-811), but that is a pure 2D `drawImage` blit of a
/// preloaded stage-select image (xtGraphics.java:2479-2481) — the fase==1
/// block paints no 3D fill at all. Our pipeline stands `Medium.d` in for
/// it, and that stand-in must paint at the pool draw's camera so the
/// ground horizon stays under the pieces; hence the step, then the fill,
/// then the pool — not the blit's literal call order.
///
/// Returns the number of pieces that went through the sort — caller-side
/// render telemetry only.
pub fn fase1(
    scene: &mut Scene,
    cv: &mut Canvas,
    m: &mut Medium,
    t: &Trackers,
    cp: &CheckPoints,
) -> usize {
    // GameSparker.java:811 — the flyover camera step, before anything
    // paints with the new camera (see the backdrop note above; the xt
    // `trackbg` 2D blit lives inside the UI milestone, still pending)
    crate::conto::aroundtrack(m, cp);
    m.d(cv);

    draw_sorted(scene, cv, m, t, 5, scene.notb)
}

/// One `fase == 0` racing run (GameSparker.java:926-1070): the five car
/// slots sit in the pool this time, so the draw scans `[0..nob)`, then the
/// countdown bookkeeping runs.
///
/// The head runs the freshly-fixed car swap (GameSparker.java:927-940):
/// with `Madness.newcar` flagged, the pool car is rebuilt from the fresh
/// model baseline in place — position kept, the placement zeroes the
/// orientation and the three original angles ride back on, and the flag
/// clears.
///
/// `fase` lives on `xt` now (the source's `var6.fase`; the stat calls
/// below transition it to fase -2/-6 like the source writes).
///
/// Remaining deferments: the `mouses` skip input of :1046.
///
/// Camera note: the original re-frames the player camera at the tail of
/// every starcnt<35 run (GameSparker.java:1030-1044) and once more inside
/// the `starcnt == 35` transition (GameSparker.java:1058-1064) — the
/// calls are idempotent re-frames, so parking the step at the run's head
/// gives every painted run the camera the previous tail left. The
/// countdown's opening `starcnt >= 36` passes keep the intro's
/// `around(car3, true)` orbit rolling (1051), and the frame right under
/// the `== 35` landing carries the landing's own follow re-frame (1064),
/// which rules every view for that frame.
///
/// Returns the number of pieces that went through the sort — telemetry.
#[allow(clippy::too_many_arguments)]
pub fn fase0(
    scene: &mut Scene,
    mad: &mut [Madness; PLAYERS],
    ctl: &mut [Control; PLAYERS],
    cp: &mut CheckPoints,
    cv: &mut Canvas,
    m: &mut Medium,
    t: &Trackers,
    rpd: &mut Record,
    starcnt: &mut i32,
    view: usize,
    newcar: &[Option<Conto>],
    xt: &mut XtGraphics,
) -> usize {
    // GameSparker.java:927-940 — the freshly-fixed car swap: a car that
    // just took a fix pad rebuilds from the fresh model baseline in
    // place; the placement zeroes the orientation and the original
    // xz/xy/zy ride back on.
    for var42 in 0..PLAYERS {
        if mad[var42].newcar {
            let var50 = scene.pool[var42].xz;
            let var57 = scene.pool[var42].xy;
            let var69 = scene.pool[var42].zy;
            let (x, y, z) = (
                scene.pool[var42].x,
                scene.pool[var42].y,
                scene.pool[var42].z,
            );
            scene.pool[var42] = Conto::clone_from(
                newcar[mad[var42].cn]
                    .as_ref()
                    .expect("fase0 rebuild for a model the race never loaded"),
                m,
                x,
                y,
                z,
                0,
            );
            scene.pool[var42].xz = var50;
            scene.pool[var42].xy = var57;
            scene.pool[var42].zy = var69;
            mad[var42].newcar = false;
        }
    }

    // GameSparker.java:942-983 — the race draw (cars included)
    m.d(cv);
    let n = draw_sorted(scene, cv, m, t, 0, scene.nob);

    // GameSparker.java:985-1015 — the per-frame physics chain at
    // starcnt == 0: the pairwise colide loop (var73 ≠ var90 order
    // pairs, both Madness/ContO halves borrowed through split_at_mut),
    // drive over all five car slots, the recorder snapshot per car,
    // checkstat, then the AI `preform` on cars 1..4. The rpd
    // pushes inside the damage registers/crash arms go to the shared
    // Record the original Madness pointers all hold.
    if *starcnt == 0 {
        for i in 0..PLAYERS {
            for j in 0..PLAYERS {
                if i != j {
                    let hi = if i < j { j } else { i };
                    let (lo_m, hi_m) = mad.split_at_mut(hi);
                    let (lo_p, hi_p) = scene.pool.split_at_mut(hi);
                    let (mi, mj) = if i < j {
                        (&mut lo_m[i], &mut hi_m[0])
                    } else {
                        (&mut hi_m[0], &mut lo_m[j])
                    };
                    let (pi, pj) = if i < j {
                        (&mut lo_p[i], &mut hi_p[0])
                    } else {
                        (&mut hi_p[0], &mut lo_p[j])
                    };
                    mi.colide(pi, mj, pj, m, rpd, xt);
                }
            }
        }
        for i in 0..PLAYERS {
            mad[i].drive(&mut ctl[i], &mut scene.pool[i], t, cp, m, rpd, xt);
        }
        // GameSparker.java:1006-1009 — the recorder's per-frame
        // snapshot (PoseRing + the 50-frame staged contour shift).
        for i in 0..PLAYERS {
            rpd.rec(
                m,
                &scene.pool[i],
                i,
                mad[i].squash,
                mad[i].lastcolido,
                mad[i].cntdest,
            );
        }
        // GameSparker.java:1010 — the placement pass over the five car
        // slots.
        cp.checkstat(mad, &scene.pool);
        // GameSparker.java:1012-1015 — the AI brain on competitor slots
        // 1..4 only; slot 0 is the player's Control.
        for i in 1..PLAYERS {
            ctl[i].preform(&mut mad[i], &scene.pool[i], cp, t, m);
        }
    } else {
        // GameSparker.java:1017-1023 — the film-roll setup as the
        // countdown opens: the wide pan, the orbit tilt and the drive
        // speed, and the white-out covers the opening frame's stale
        // aim (the xt `stat` HUD paint rides the same white). It runs
        // BEFORE the countdown step and the camera dispatch, whose
        // post-decrement value the source's `around(car3, true)` reads.
        if *starcnt == 90 {
            m.adv = 1900;
            m.zy = 40;
            m.vxz = 70;
            cv.fill([255, 255, 255]);
        }

        // GameSparker.java:1025-1027 — the countdown wears on.
        if *starcnt != 0 {
            *starcnt -= 1;
        }
    }

    // The source's counter lives on xtGraphics (the tests write it
    // directly); sync the field with the threaded local before the
    // camera/HUD reads.
    xt.starcnt = *starcnt;

    // GameSparker.java:1029-1068 — the countdown half of the chain: below
    // 35 the view's camera and the xt `stat` HUD run; at/above it the
    // intro's `around(car3, true)` orbit rolls, a press clamps the
    // countdown to 35 and the landing frame re-rolls the sweep onto the
    // player's film (adv 900, vxz 180, vert off), checks the placements
    // once, paints the `stat` HUD (the follow view) and whites the frame.
    if *starcnt < 35 {
        match view {
            0 => {
                crate::conto::follow(m, &scene.pool[0], mad[0].cxz);
                xt.stat(&mut mad[0], cp, &mut ctl[0], true, cv, m);
            }
            1 => {
                crate::conto::around(m, &scene.pool[0], false);
                xt.stat(&mut mad[0], cp, &mut ctl[0], false, cv, m);
            }
            2 => {
                crate::conto::watch(m, &scene.pool[0], mad[0].mxz);
                xt.stat(&mut mad[0], cp, &mut ctl[0], false, cv, m);
            }
            _ => {}
        }
    } else {
        crate::conto::around(m, &scene.pool[3], true);
        if ctl[0].enter || ctl[0].handb {
            *starcnt = 35;
            ctl[0].enter = false;
            ctl[0].handb = false;
        }

        if *starcnt == 35 {
            m.vert = false;
            m.adv = 900;
            m.vxz = 180;
            cp.checkstat(mad, &scene.pool);
            crate::conto::follow(m, &scene.pool[0], mad[0].cxz);
            xt.starcnt = *starcnt;
            xt.stat(&mut mad[0], cp, &mut ctl[0], true, cv, m);
            cv.fill([255, 255, 255]);
        }
    }

    xt.starcnt = *starcnt;
    n
}

/// The race-end replay, `fase == -1` (GameSparker.java:1072-1167), one
/// source pass per call with `replay` holding the frame counter `var22`
/// the Java local keeps across the if-chain. Frame 0 seeds `ocar` with
/// the race's end-state pool clones and swaps the pool cars for the
/// staged snapshot clones (`Record.car[0][..]`, position 0); `play`
/// then re-poses each car per frame off the pose ring. The destruct
/// `fcnt` checks rebuild a car from the fresh model baseline
/// (`newcar[cn]` — the port of the `var8` array, a 43-strong null row
/// until the caller fills the model ids the race uses), `replay == 299`
/// restores the seeded end state, and the 300th frame wraps the counter
/// back to 0 (the shell reads the wrap as the `fase == -6` handoff).
///
/// `newcar` is indexed by car model id (`mad[cn]`) like the Java array,
/// not by pool slot — cars sharing a model share one baseline.
///
/// The 300-wrap resets the counter here; the block tail (the `fase == -6`
/// handoff with the audio track stop, the `replyn` bar paint, the
/// `around` camera swing) rides with the shell dispatch
/// (`shell::Shell::frame`). The `mouses` skip input rides with the
/// real-time shell.
pub fn fasereplay1(
    scene: &mut Scene,
    mad: &mut [Madness; PLAYERS],
    ctl: &mut [Control; PLAYERS],
    cv: &mut Canvas,
    m: &mut Medium,
    t: &Trackers,
    rpd: &mut Record,
    replay: &mut i32,
    newcar: &[Option<Conto>],
) -> usize {
    // GameSparker.java:1072-1080 — the frame-0 seed: ocar takes the
    // end-state pool clone, the pool car becomes the staged snapshot
    // clone (position 0; `play` re-poses from the ring row below).
    if *replay == 0 {
        for var44 in 0..PLAYERS {
            let end = Conto::clone_from(&scene.pool[var44], m, 0, 0, 0, 0);
            rpd.ocar[var44] = Some(end);
            let staged = Conto::clone_from(
                rpd.car[0][var44]
                    .as_ref()
                    .expect("replay before the recorder reset"),
                m,
                0,
                0,
                0,
                0,
            );
            scene.pool[var44] = staged;
        }
    }

    // GameSparker.java:1082-1123 — the backdrop and the pool draw with
    // the painter's count sort (same roster the fase == 0 pass uses).
    m.d(cv);
    let n = draw_sorted(scene, cv, m, t, 0, scene.nob);

    // GameSparker.java:1125-1130 — the skip arm jumps the film to the
    // last frame (the xt mouse click joins the same arm).
    if ctl[0].enter || ctl[0].handb {
        *replay = 299;
        ctl[0].enter = false;
        ctl[0].handb = false;
    }

    for var80 in 0..PLAYERS {
        // GameSparker.java:1135-1141 — the recorded fix stamp frame:
        // cars behind the camera rebuild at once (`fcnt` 8), the rest
        // carry the fix tag visually.
        if rpd.fix[var80] == *replay {
            if scene.pool[var80].dist == 0 {
                scene.pool[var80].fcnt = 8;
            } else {
                scene.pool[var80].fix = true;
            }
        }

        // GameSparker.java:1143-1146 — the fixed-car rebuild from the
        // fresh model baseline.
        if scene.pool[var80].fcnt == 7 || scene.pool[var80].fcnt == 8 {
            scene.pool[var80] = Conto::clone_from(
                newcar[mad[var80].cn]
                    .as_ref()
                    .expect("replay rebuild for a model the film rig never loaded"),
                m,
                0,
                0,
                0,
                0,
            );
            rpd.cntdest[var80] = 0;
        }

        // GameSparker.java:1148-1150 — the last frame restores the
        // seeded end state (its pose bits get re-posed right after; the
        // plane dent/chip state is what survives).
        if *replay == 299 {
            scene.pool[var80] = Conto::clone_from(
                rpd.ocar[var80]
                    .as_ref()
                    .expect("replay frame 299 before the frame-0 seed"),
                m,
                0,
                0,
                0,
                0,
            );
        }

        // GameSparker.java:1152 — the replay painter on the shared
        // Madness stat holder.
        rpd.play(&mut scene.pool[var80], &mad[var80], var80, *replay, m);
    }

    // GameSparker.java:1155-1164 — the 300-frame loop: the wrap hands
    // off to the result screen, otherwise the xt reply bar paints.
    *replay += 1;
    if *replay == 300 {
        *replay = 0;
    }

    n
}

/// The wasted crash-cam handoff, `fase == -2` (GameSparker.java:1169-1188),
/// one call at the post-race crossroads: with a recorded wasted event the
/// Medium picks up its film-roll sweep parameters (a fresh `random()`
/// settles the sweep orientation and its pan length, the tilt speed is
/// the constant 180), the film counter clears and the `hold`/`sweeps`
/// counters reset — the crash-cam `fase == -3` pass takes over next
/// call; without one the trackbg backdrop paints and the shell falls
/// through to the result screen at `replay == -2`.
pub fn fasewasted2(
    m: &mut Medium,
    cv: &mut Canvas,
    rpd: &Record,
    replay: &mut i32,
    hold: &mut i32,
    sweeps: &mut i32,
    xt: &mut XtGraphics,
) {
    if rpd.hcaught {
        // GameSparker.java:1171-1178 — the cinematic sweep parameters:
        // `random() > 0.45` maps to `vert = false`, else `true` (the
        // `<=` collapses the if/else; the stream never yields NaN, so
        // the two read identically).
        m.vert = m.random() <= 0.45;
        m.adv = (900.0f32 * m.random()) as i32;
        m.vxz = 180;
        *replay = 0;
        xt.fase = -3;
        *hold = 0;
        *sweeps = 0;
    } else {
        // GameSparker.java:1184-1186 — the trackbg blit, then the result
        // screen at replay -2.
        cv.draw_image(&xt.trackbg, 0, 0);
        *replay = -2;
        xt.fase = -4;
    }
}

/// The wasted crash-cam film, `fase == -3` (GameSparker.java:1190-1316),
/// one source pass per call — `replay` is the film counter `var22`,
/// `hold` the crash-moment hold counter `var23`, `sweeps` the screening
/// counter `var24`, `xt.fase` the shell's phase. Frame 0 re-seeds every
/// pool slot from the `starcar` snapshot clones, then each pass runs
/// the count-sorted draw, the `hfix` stamp / `fcnt` rebuild / `playh`
/// chain per car, and the film body.
///
/// The film body freezes the counter once it passes the recorded crash
/// moment (`whenwasted`): with a wasted competitor the camera swings
/// from the player over onto the wreck (`hold` climbing to 20, the
/// white flash at 10); with the player wasted the fixed camera fills
/// white at hold 3, 31 and 66 on its way to 67. While `hold` is away
/// from its gate value the counter parks, so the crash frame stays on
/// screen for the whole swing. A full 300-frame screening counts one
/// sweep in `sweeps`; the end of the second screening stamps the skip
/// key for the caller's shell and the same pass exits through the skip
/// arm to `fase == -4` with `replay == -7`. An enter/handb press exits
/// early the same way.
///
/// The headline paint (`xtGraphics.levelhigh`, GameSparker.java:1266) runs
/// every non-skipped frame before the fade-in black.
pub fn fasewasted3(
    scene: &mut Scene,
    mad: &mut [Madness; PLAYERS],
    ctl: &mut [Control; PLAYERS],
    cv: &mut Canvas,
    m: &mut Medium,
    t: &Trackers,
    rpd: &mut Record,
    replay: &mut i32,
    hold: &mut i32,
    sweeps: &mut i32,
    xt: &mut XtGraphics,
    newcar: &[Option<Conto>],
) -> usize {
    // GameSparker.java:1191-1197 — the frame-0 seed: the pool re-bases
    // on the starcar snapshot clones (position 0; playh re-poses from
    // the h-ring rows below).
    if *replay == 0 {
        for var46 in 0..PLAYERS {
            scene.pool[var46] = Conto::clone_from(
                rpd.starcar[var46]
                    .as_ref()
                    .expect("crash-cam before the recorder reset"),
                m,
                0,
                0,
                0,
                0,
            );
        }
    }

    // GameSparker.java:1199-1240 — the backdrop and the pool draw with
    // the painter's count sort (same roster the other fase passes use).
    m.d(cv);
    let n = draw_sorted(scene, cv, m, t, 0, scene.nob);

    for var84 in 0..PLAYERS {
        // GameSparker.java:1245-1251 — the recorded fix stamp frame:
        // cars behind the camera rebuild at once (`fcnt` 8), the rest
        // carry the fix tag visually.
        if rpd.hfix[var84] == *replay {
            if scene.pool[var84].dist == 0 {
                scene.pool[var84].fcnt = 8;
            } else {
                scene.pool[var84].fix = true;
            }
        }

        // GameSparker.java:1253-1256 — the fixed-car rebuild from the
        // fresh model baseline.
        if scene.pool[var84].fcnt == 7 || scene.pool[var84].fcnt == 8 {
            scene.pool[var84] = Conto::clone_from(
                newcar[mad[var84].cn]
                    .as_ref()
                    .expect("crash-cam rebuild for a model the film rig never loaded"),
                m,
                0,
                0,
                0,
                0,
            );
            rpd.cntdest[var84] = 0;
        }

        // GameSparker.java:1258 — the wasted-replay painter on the
        // shared Madness stat holder.
        rpd.playh(&mut scene.pool[var84], &mad[var84], var84, *replay, m);
    }

    // GameSparker.java:1261-1263 — the second screening's last frame
    // stamps the skip key itself.
    if *sweeps == 2 && *replay == 299 {
        ctl[0].enter = true;
    }

    // GameSparker.java:1265-1315 — the film body: no skip key drives it.
    if !ctl[0].enter && !ctl[0].handb {
        // GameSparker.java:1266 — the xt headline (levelhigh) and the
        // fade-in: the first three frames paint pure black over the scene.
        xt.levelhigh(cv, rpd.wasted, rpd.whenwasted, m);
        if (0..=2).contains(replay) {
            cv.fill([0, 0, 0]);
        }

        if rpd.wasted != 0 {
            // GameSparker.java:1273-1276 — the swing flash.
            if *hold == 10 {
                cv.fill([255, 255, 255]);
            }

            // GameSparker.java:1278-1282 — the camera: onto the wreck
            // once the hold closes in, the player before.
            if *hold >= 10 {
                crate::conto::around(m, &scene.pool[rpd.wasted as usize], false);
            } else {
                crate::conto::around(m, &scene.pool[0], false);
            }

            // GameSparker.java:1284-1286 — the hold climbs past the
            // recorded crash moment and parks at 20.
            if *replay > rpd.whenwasted && *hold != 20 {
                *hold += 1;
            }

            // GameSparker.java:1288-1292 — the film only advances while
            // the camera sits at either end of the swing.
            if *hold == 0 || *hold == 20 {
                *replay += 1;
                if *replay == 300 {
                    *replay = 0;
                    *hold = 0;
                    *sweeps += 1;
                }
            }
        } else {
            // GameSparker.java:1294-1297 — the triple flash.
            if *hold == 3 || *hold == 31 || *hold == 66 {
                cv.fill([255, 255, 255]);
            }

            // GameSparker.java:1299-1301 — the hold climbs past the
            // crash moment and parks at 67.
            if *replay > rpd.whenwasted && *hold != 67 {
                *hold += 1;
            }

            // GameSparker.java:1303 — the player stays on camera.
            crate::conto::around(m, &scene.pool[0], false);

            // GameSparker.java:1304-1308 — the film only advances while
            // the flash counter sits at 0 or 67.
            if *hold == 0 || *hold == 67 {
                *replay += 1;
                if *replay == 300 {
                    *replay = 0;
                    *hold = 0;
                    *sweeps += 1;
                }
            }
        }
    } else {
        // GameSparker.java:1310-1315 — the skip arm: the film ends, the
        // shell drops on the result screen (xt.fase, the source's
        // var6.fase write).
        xt.fase = -4;
        ctl[0].enter = false;
        ctl[0].handb = false;
        *replay = -7;
    }

    n
}

/// One `fase == -4` pass (GameSparker.java:1318-1345): the result
/// fade. The first pass draws the `mdness` plate at (164,330), then
/// `fleximage` runs over the live frame with the shared `var22`
/// counter (threaded as `hold`) — the grab + re-hue at 0, the
/// darkening passes 1..5 — with the result banner (`congrd` at
/// (140,30) for a newest-stage win, (140,117) for an older one;
/// `gameov` at (190,117) for a loss) riding from the second pass. The
/// sixth pass closes the fade into fase -5.
pub fn faseresult4(
    xt: &mut XtGraphics,
    cv: &mut Canvas,
    m: &mut Medium,
    cp: &CheckPoints,
    hold: &mut i32,
    ctl: &mut Control,
) {
    if *hold <= 0 {
        cv.draw_image(&xt.mdness, 164, 330);
    }
    if *hold >= 0 {
        xt.fleximage(cv, m, *hold);
        if *hold != 0 {
            if xt.winner {
                if cp.stage == xt.unlocked {
                    cv.draw_image(&xt.congrd, 140, 30);
                    if ctl.enter || ctl.handb {
                        ctl.enter = false;
                        ctl.handb = false;
                    }
                } else {
                    cv.draw_image(&xt.congrd, 140, 117);
                }
            } else {
                cv.draw_image(&xt.gameov, 190, 117);
            }
        }
    }
    *hold += 1;
    if *hold == 6 {
        xt.fase = -5;
    }
}

/// One `fase == -8` pass (GameSparker.java:1369-1382): the
/// not-enough-replay notice (`cantreply`) holds for 150 frames — the
/// source's shared `var22` counter, threaded here as `hold` — or until
/// enter/handb or a mouse press; the hold then ends by restoring the
/// paused menu frame (the `fleximg` backdrop plus the `paused` plate)
/// and handing back to the pause screen (fase -7). The caller's mouse
/// state reset (`this.mouses = 0`) belongs to the shell loop.
pub fn fasecantreply(
    xt: &mut XtGraphics,
    cv: &mut Canvas,
    m: &Medium,
    hold: &mut i32,
    ctl: &mut Control,
    mouses: i32,
) {
    xt.cantreply(cv, m);
    *hold += 1;
    if *hold == 150 || ctl.enter || ctl.handb || mouses == 1 {
        cv.draw_image(&xt.fleximg, 0, 0);
        cv.draw_image(&xt.paused, 156, 106);
        xt.fase = -7;
        ctl.enter = false;
        ctl.handb = false;
    }
}

#[cfg(test)]
mod tests {
    /// A Medium with a private, seeded Java stream (C3b: the default now
    /// reads the process-global Math.random() slot; the replay rigs pair
    /// a target with a probe, so each needs its own seeded stream).
    fn seeded_medium() -> Medium {
        let mut m = Medium::new();
        m.rng = Some(crate::random::RngSource::Java(
            crate::random::JavaRandom::new(42),
        ));
        m
    }

    use super::*;
    use crate::canvas::Canvas;
    use crate::checkpoints::CheckPoints;
    use crate::conto::{Base, Conto};
    use crate::medium::Medium;
    use crate::trackers::Trackers;

    fn replay_rig() -> (Scene, Medium, Trackers, CheckPoints) {
        let m = seeded_medium();
        let mut t = Trackers::new();
        let mut base = Base {
            keyx: [10, -10, 10, -10],
            keyz: [10, 10, -10, -10],
            ..Default::default()
        };
        // One body plane (wz == 0) so the fleet carries paint state the
        // replay can wear and rebuild; vertex 0 rides the wheel-0 key
        // anchor like record.rs's painter fixture.
        use crate::plane::Plane;
        base.planes.push(Plane::new(
            &m,
            &[0, 100, 100],
            &[0, 100, 100],
            &[-100, -100, -100],
            3,
            [120, 130, 140],
            false,
            10,
            0,
            0,
            0,
            7,
            5,
            false,
        ));
        let mut pool = Vec::new();
        for _ in 0..crate::checkpoints::PLAYERS {
            pool.push(Conto::place(&base, &m, &mut t, 0, 0, 0, 0));
        }
        (
            Scene {
                pool,
                notb: 5,
                nob: 5,
                trx: 0,
                trz: 0,
            },
            m,
            t,
            CheckPoints::new(),
        )
    }

    fn newcar_of(pool0: &Conto, m: &Medium) -> Vec<Option<Conto>> {
        // `ContO[] var8 = new ContO[43]` (GameSparker.java:594): one slot
        // per model id, filled here for the cn the rig races (0).
        let mut v: Vec<Option<Conto>> = (0..43).map(|_| None).collect();
        v[0] = Some(Conto::clone_from(pool0, m, 0, 0, 0, 0));
        v
    }

    fn replay_state() -> ([Madness; PLAYERS], [Control; PLAYERS], Record) {
        let ctl = [
            Control::new(),
            Control::new(),
            Control::new(),
            Control::new(),
            Control::new(),
        ];
        let mad = [
            Madness::new(0),
            Madness::new(1),
            Madness::new(2),
            Madness::new(3),
            Madness::new(4),
        ];
        (mad, ctl, Record::new())
    }

    fn prime_starcar(rpd: &mut Record, pool: &[Conto], m: &Medium) {
        // cotchinow's snapshot arm (Record.java:658): one starcar clone
        // per slot, taken exactly as the wasted event leaves them.
        for slot in 0..PLAYERS {
            rpd.starcar[slot] = Some(Conto::clone_from(&pool[slot], m, 0, 0, 0, 0));
        }
    }

    /// One `fase == 0` racing run the swap test drives with a mid-count
    /// starcnt so the physics chain stays off.
    #[test]
    fn fase0_swaps_newcar_slots_in_place() {
        let (mut scene, mut m, t, mut cp) = replay_rig();
        let (mut mad, mut u, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);

        // A fix pad flagged slot 3 for a fresh body: the swap keeps the
        // position and the three angles, rebuilds the plane state and
        // clears the flag in the same pass.
        scene.pool[3].planes[0].bfase = 44;
        scene.pool[3].xz = 123;
        scene.pool[3].xy = 45;
        scene.pool[3].zy = 66;
        scene.pool[3].x = 4000;
        scene.pool[3].y = 250;
        scene.pool[3].z = -7000;
        mad[3].newcar = true;

        let mut starcnt = 5i32;
        let mut xt = XtGraphics::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        fase0(
            &mut scene,
            &mut mad,
            &mut u,
            &mut cp,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut starcnt,
            0,
            &newcar,
            &mut xt,
        );
        assert!(!mad[3].newcar, "the swap cleared the flag");
        assert_eq!(scene.pool[3].planes[0].bfase, 5, "rebuild cleared the wear");
        // Position and orientation survive the rebuild.
        assert_eq!(scene.pool[3].x, 4000);
        assert_eq!(scene.pool[3].y, 250);
        assert_eq!(scene.pool[3].z, -7000);
        assert_eq!(scene.pool[3].xz, 123);
        assert_eq!(scene.pool[3].xy, 45);
        assert_eq!(scene.pool[3].zy, 66);
        assert_eq!(starcnt, 4, "the countdown ticked");
        assert_eq!(scene.pool[0].planes[0].bfase, 5);
        assert_eq!(scene.pool[0].x, 0, "unflagged slots untouched");
    }

    #[test]
    fn wasted2_arms_the_crash_cam_from_the_recorded_wasted() {
        let (_, mut m, _t, _cp) = replay_rig();
        let (_, _, mut rpd) = replay_state();
        rpd.hcaught = true;

        // A deterministic stream: the sweep rolls its orientation and
        // its pan off exactly two `random()`s, the tilt is the constant
        // 180 and the film counters all clear.
        let mut probe = seeded_medium();
        let sweep_vert_random = probe.random();
        let sweep_adv_random = probe.random();

        let mut replay = 123i32;
        let mut hold = 4i32;
        let mut sweeps = 9i32;
        let mut xt = XtGraphics::new();
        let mut cv = Canvas::new(550, 400);
        fasewasted2(
            &mut m,
            &mut cv,
            &rpd,
            &mut replay,
            &mut hold,
            &mut sweeps,
            &mut xt,
        );

        assert_eq!(xt.fase, -3, "the film armed");
        assert_eq!(replay, 0, "the counter cleared");
        assert_eq!(hold, 0);
        assert_eq!(sweeps, 0);
        assert_eq!(m.vert, sweep_vert_random <= 0.45, "the sweep orientation");
        assert_eq!(m.adv, (900.0f32 * sweep_adv_random) as i32, "the sweep pan");
        assert_eq!(m.vxz, 180);
    }

    #[test]
    fn wasted2_without_a_caught_crash_falls_to_the_result_screen() {
        let (_, mut m, _t, _cp) = replay_rig();
        let (_, _, rpd) = replay_state();

        // No wasted event recorded: the film is skipped (the trackbg
        // backdrop paints — a no-op over the rig's empty sprite) and the
        // shell drops on the result screen at replay -2, leaving the
        // hold/sweeps counters and the Medium stream alone.
        let mut probe = seeded_medium();

        let mut replay = 0i32;
        let mut hold = 5i32;
        let mut sweeps = 5i32;
        let mut xt = XtGraphics::new();
        let mut cv = Canvas::new(550, 400);
        fasewasted2(
            &mut m,
            &mut cv,
            &rpd,
            &mut replay,
            &mut hold,
            &mut sweeps,
            &mut xt,
        );

        assert_eq!(xt.fase, -4);
        assert_eq!(replay, -2);
        assert_eq!(hold, 5, "the arm left the hold alone");
        assert_eq!(sweeps, 5, "the arm left the sweeps alone");
        assert_eq!(m.random(), probe.random(), "no randoms rolled");
    }

    #[test]
    fn wasted3_seeds_the_pool_from_starcar_and_replays_off_the_h_rings() {
        let (mut scene, mut m, t, _cp) = replay_rig();
        let (mut mad, mut ctl, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        prime_starcar(&mut rpd, &scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);

        // The h-ring carries the film's frame-0 pose for slot 2.
        rpd.hx[0][2] = 7777;
        rpd.hz[0][2] = -500;

        let mut replay = 0i32;
        let mut hold = 0i32;
        let mut sweeps = 0i32;
        let mut xt = XtGraphics::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        fasewasted3(
            &mut scene,
            &mut mad,
            &mut ctl,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut replay,
            &mut hold,
            &mut sweeps,
            &mut xt,
            &newcar,
        );

        assert_eq!(replay, 1, "frame 0 advanced");
        for slot in 0..PLAYERS {
            assert!(rpd.starcar[slot].is_some(), "the film ran off snapshots");
        }
        // The frame-0 seed re-based every slot at position 0 and then
        // playh set the ring pose back on slot 2.
        assert_eq!(scene.pool[1].x, 0);
        assert_eq!(scene.pool[2].x, 7777, "the h-ring re-pose");
        assert_eq!(scene.pool[2].z, -500);
        assert_eq!(hold, 0, "the crash hold has not started");
    }

    #[test]
    fn wasted3_holds_the_film_at_the_crash_moment_and_double_sweeps() {
        let (mut scene, mut m, t, _cp) = replay_rig();
        let (mut mad, mut ctl, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        prime_starcar(&mut rpd, &scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);

        // A competitor wreck at the recorded ring moment 5: the film
        // parks at frame 6 while the hold climbs to 20 (the white flash
        // lands on the first wreck-camera frame), both screenings walk
        // their full ring and the second one hands the screen over.
        rpd.wasted = 2;
        rpd.whenwasted = 5;

        let mut replay = 0i32;
        let mut hold = 0i32;
        let mut sweeps = 0i32;
        let mut xt = XtGraphics::new();

        let mut frozen = 0usize;
        let mut flashes = 0usize;
        let mut fades = 0usize;
        for _ in 0..1400 {
            let mut cv = Canvas::new(m.w as usize, m.h as usize);
            fasewasted3(
                &mut scene,
                &mut mad,
                &mut ctl,
                &mut cv,
                &mut m,
                &t,
                &mut rpd,
                &mut replay,
                &mut hold,
                &mut sweeps,
                &mut xt,
                &newcar,
            );
            if (1..20).contains(&hold) && replay == 6 {
                frozen += 1;
            }
            if cv.px.iter().all(|&p| p == 255) {
                flashes += 1;
            }
            if cv.px.iter().all(|&p| p == 0) {
                fades += 1;
            }
            if xt.fase == -4 {
                break;
            }
        }

        assert_eq!(xt.fase, -4, "the double sweep ended the film");
        assert_eq!(replay, -7, "the skip arm's exit stamp");
        assert_eq!(sweeps, 2, "the two screening wrap");
        assert!(!ctl[0].enter, "the auto-enter was consumed");
        assert!(!ctl[0].handb);
        // Three screenings: 19 parked passes while each hold climbs to
        // 20, one white flash per climb and the fade over each
        // screening's first three frames.
        assert_eq!(frozen, 57, "the crash-moment holds");
        assert_eq!(flashes, 3, "the swing flashes");
        assert_eq!(fades, 9, "the fade-in frames");
    }

    #[test]
    fn wasted3_player_wasted_keeps_the_fixed_camera_and_the_triple_flash() {
        let (mut scene, mut m, t, _cp) = replay_rig();
        let (mut mad, mut ctl, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        prime_starcar(&mut rpd, &scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);

        // The player's own wreck: the camera stays on the player, the
        // film parks at the recorded moment 3 for the triple flash's
        // 67-pass climb.
        rpd.wasted = 0;
        rpd.whenwasted = 3;

        let mut replay = 0i32;
        let mut hold = 0i32;
        let mut sweeps = 0i32;
        let mut xt = XtGraphics::new();

        let mut frozen = 0usize;
        let mut flashes = 0usize;
        for _ in 0..1500 {
            let mut cv = Canvas::new(m.w as usize, m.h as usize);
            fasewasted3(
                &mut scene,
                &mut mad,
                &mut ctl,
                &mut cv,
                &mut m,
                &t,
                &mut rpd,
                &mut replay,
                &mut hold,
                &mut sweeps,
                &mut xt,
                &newcar,
            );
            if (1..67).contains(&hold) && replay == 4 {
                frozen += 1;
            }
            if cv.px.iter().all(|&p| p == 255) {
                flashes += 1;
            }
            if xt.fase == -4 {
                break;
            }
        }

        assert_eq!(xt.fase, -4, "the double sweep ended the film");
        assert_eq!(replay, -7);
        assert_eq!(sweeps, 2);
        // Three screenings, each parking 66 passes on the climb with the
        // three white fills of holds 3, 31 and 66.
        assert_eq!(frozen, 198, "the crash-moment holds");
        assert_eq!(flashes, 9, "the triple flashes");
    }

    #[test]
    fn wasted3_rebuilds_fresh_cells_and_arms_the_fix_stamp() {
        let (mut scene, mut m, t, _cp) = replay_rig();
        let (mut mad, mut ctl, mut rpd) = replay_state();
        let newcar = newcar_of(&scene.pool[0], &m);

        // Mid-film: the frame-0 seed re-clones from starcar and Java's
        // copy constructor zeroes fcnt, so the rebuild gate is stamped
        // here directly — a wear-stained, half-dead slot 3 (fcnt 7, the
        // chain's rebuild state) with a pending destruct reply — and
        // slot 2's fix stamp is recorded for this very frame.
        scene.pool[3].planes[0].bfase = 44;
        scene.pool[3].fcnt = 7;
        rpd.cntdest[3] = 9;
        rpd.hfix[2] = 130;

        let mut replay = 130i32;
        let mut hold = 0i32;
        let mut sweeps = 0i32;
        let mut xt = XtGraphics::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        fasewasted3(
            &mut scene,
            &mut mad,
            &mut ctl,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut replay,
            &mut hold,
            &mut sweeps,
            &mut xt,
            &newcar,
        );

        assert_eq!(
            scene.pool[3].planes[0].bfase, 5,
            "rebuilt from the fresh baseline"
        );
        assert_eq!(scene.pool[3].fcnt, 0, "the cell cleared");
        assert_eq!(rpd.cntdest[3], 0);
        // The fix arm branches on this pass's draw: behind the camera is
        // the at-once rebuild state, ahead carries the visual tag.
        assert_eq!(scene.pool[2].fix, scene.pool[2].dist != 0);
    }

    #[test]
    fn wasted3_skips_out_on_a_press() {
        let (mut scene, mut m, t, _cp) = replay_rig();
        let (mut mad, mut ctl, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        prime_starcar(&mut rpd, &scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);

        // A pressed handbrake on the first frame: the film paints its
        // frame anyway and the skip arm closes it out.
        ctl[0].handb = true;
        let mut replay = 0i32;
        let mut hold = 0i32;
        let mut sweeps = 0i32;
        let mut xt = XtGraphics::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        fasewasted3(
            &mut scene,
            &mut mad,
            &mut ctl,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut replay,
            &mut hold,
            &mut sweeps,
            &mut xt,
            &newcar,
        );

        assert_eq!(xt.fase, -4);
        assert_eq!(replay, -7);
        assert!(!ctl[0].handb, "the skip arm cleared the press");
        assert_eq!(hold, 0, "the film body never ran");
        assert_eq!(sweeps, 0);
    }

    #[test]
    fn fase0_intro_orbit_winds_up_and_the_35_landing_whites_out() {
        let (mut scene, mut m, t, mut cp) = replay_rig();
        let (mut mad, mut u, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);

        // Distinct clear ranks so the landing's placement check ranks
        // ties directly (the bare rig carries no gate field to walk).
        for slot in 0..PLAYERS {
            mad[slot].clear = slot as i32;
        }

        // The countdown opens at 90: the film-roll sweep arms wide over
        // a white-out, then the SAME pass's orbit around car 3 pulls the
        // aim in one step (GameSparker.java:1017-1044 runs the roll setup,
        // the countdown tick and then `around(car3, true)`; the port
        // follows that order — the mirror's matrix pins 1880/74).
        let mut starcnt = 90i32;
        let mut xt = XtGraphics::new();
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        fase0(
            &mut scene,
            &mut mad,
            &mut u,
            &mut cp,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut starcnt,
            0,
            &newcar,
            &mut xt,
        );
        assert_eq!(starcnt, 89, "the countdown started wearing");
        assert_eq!(m.adv, 1880, "the opening roll armed, then one orbit step");
        assert_eq!(m.zy, 38, "the orbit tilt, one around step in");
        assert_eq!(m.vxz, 74, "the roll drive speed plus one orbit step");
        assert!(cv.px.iter().all(|&p| p == 255), "the opening white-out");

        // The orbit keeps winding in; no landing or setup arm re-fires.
        fase0(
            &mut scene,
            &mut mad,
            &mut u,
            &mut cp,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut starcnt,
            0,
            &newcar,
            &mut xt,
        );
        assert_eq!(starcnt, 88);
        assert_eq!(m.adv, 1860, "the intro orbit wound in");
        assert_eq!(m.vxz, 78, "the roll drive picked up its 4-per-pass tick");

        // A pressed enter: the skip arm clamps the countdown to 35 and
        // the landing fires the same pass — the sweep hands back to the
        // player's film roll (vert off, adv 900, vxz 180), the
        // placements check once and the frame whites out.
        u[0].enter = true;
        fase0(
            &mut scene,
            &mut mad,
            &mut u,
            &mut cp,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut starcnt,
            0,
            &newcar,
            &mut xt,
        );
        assert_eq!(starcnt, 35, "the skip clamped the countdown");
        assert_eq!(m.adv, 900, "the landing re-rolled the sweep");
        assert_eq!(m.vxz, 180);
        assert!(!m.vert, "the landing dropped the vertical sweep");
        assert!(!u[0].enter, "the skip arm cleared the press");
        assert!(cv.px.iter().all(|&p| p == 255), "the landing white-out");

        // The frame under 35 draws off the follow camera again and the
        // landing never re-fires; the roll stays put under the follow.
        fase0(
            &mut scene,
            &mut mad,
            &mut u,
            &mut cp,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut starcnt,
            0,
            &newcar,
            &mut xt,
        );
        assert_eq!(starcnt, 34, "the countdown resumed under 35");
        assert_eq!(m.adv, 900, "the follow view left the roll alone");
    }

    #[test]
    fn replay1_seeds_ocar_and_reposes_from_the_ring() {
        let (mut scene, mut m, t, _cp) = replay_rig();
        let (mut mad, mut ctl, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);
        // The staged snapshot rows are position-0 clones of the placed
        // pool: the frame-0 pool swap takes them, then play re-poses off
        // ring row 0.
        rpd.x[0][2] = 777;
        rpd.zy[0][2] = 33;

        let mut replay = 0i32;
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        fasereplay1(
            &mut scene,
            &mut mad,
            &mut ctl,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut replay,
            &newcar,
        );
        assert_eq!(replay, 1, "the film advanced one frame");
        for slot in 0..PLAYERS {
            assert!(rpd.ocar[slot].is_some(), "ocar seeded at frame 0");
        }
        assert_eq!(scene.pool[2].x, 777);
        assert_eq!(scene.pool[2].zy, 33);
        assert_eq!(scene.pool[0].x, 0, "unprinted ring rows keep pose 0");
    }

    #[test]
    fn replay1_skips_to_the_end_and_restores_the_seed() {
        let (mut scene, mut m, t, _cp) = replay_rig();
        let (mut mad, mut ctl, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);

        // Frame 0 runs first (the natural film start): ocar takes the
        // race's end state and the pool swaps onto the staged rows. A
        // wear-stained plane marks the worn car (44) that the staged
        // rows (5, primed at reset) do not carry.
        scene.pool[3].planes[0].bfase = 44;
        let mut replay = 0i32;
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        fasereplay1(
            &mut scene,
            &mut mad,
            &mut ctl,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut replay,
            &newcar,
        );
        assert_eq!(replay, 1);
        assert_eq!(
            scene.pool[3].planes[0].bfase, 5,
            "pool swapped to the staged rows"
        );

        // Now the skip: enter lands the 299 restore in the same pass,
        // the painter runs the end frame and the wrap clears.
        ctl[0].enter = true;
        fasereplay1(
            &mut scene,
            &mut mad,
            &mut ctl,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut replay,
            &newcar,
        );
        assert_eq!(replay, 0, "the film finished and wrapped");
        assert!(!ctl[0].enter, "the skip arm cleared itself");
        assert_eq!(
            scene.pool[3].planes[0].bfase, 44,
            "ocar's plane state restored"
        );
        // The pose re-applies off ring row 299 (zeroed here).
        assert_eq!(scene.pool[3].x, 0);
    }

    #[test]
    fn replay1_rebuilds_fresh_cars_on_the_fcnt_checks() {
        let (mut scene, mut m, t, _cp) = replay_rig();
        let (mut mad, mut ctl, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);

        // A mid-film fix: the fresh baseline's bfase differs from the
        // race-worn pool car (all slots carry bfase 5 here, so stamp a
        // wear mark the rebuild must clear).
        scene.pool[4].planes[0].bfase = 44;
        scene.pool[4].fcnt = 7;
        let mut replay = 130i32;
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        fasereplay1(
            &mut scene,
            &mut mad,
            &mut ctl,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut replay,
            &newcar,
        );
        assert_eq!(scene.pool[4].fcnt, 0, "the fresh Conto's fix count");
        assert_eq!(
            scene.pool[4].planes[0].bfase, 5,
            "plane bfase back to the baseline"
        );
        assert_eq!(rpd.cntdest[4], 0);
        assert_eq!(scene.pool[0].x, 0, "non-fixed slots untouched");
    }

    #[test]
    fn replay1_fix_stamp_arms_the_visual_tag() {
        let (mut scene, mut m, t, _cp) = replay_rig();
        let (mut mad, mut ctl, mut rpd) = replay_state();
        rpd.reset(&scene.pool, &m);
        let newcar = newcar_of(&scene.pool[0], &m);

        // The race stamped a mid-film fix at frame 90: it decays with
        // the racing rec() calls, so stamp it fresh for the film. A
        // wear-stained plane marks the race-worn body: the cull branch
        // (dist == 0) rebuilds from the fresh baseline and clears it,
        // the in-front branch carries the fix tag and keeps it.
        scene.pool[3].planes[0].bfase = 44;
        rpd.fix[3] = 90;
        let mut replay = 90i32;
        let mut cv = Canvas::new(m.w as usize, m.h as usize);
        fasereplay1(
            &mut scene,
            &mut mad,
            &mut ctl,
            &mut cv,
            &mut m,
            &t,
            &mut rpd,
            &mut replay,
            &newcar,
        );
        let carried = scene.pool[3].dist != 0;
        if carried {
            assert!(scene.pool[3].fix, "in-front car arms the fix tag");
            assert_eq!(scene.pool[3].planes[0].bfase, 44, "no rebuild");
        } else {
            assert_eq!(scene.pool[3].planes[0].bfase, 5, "culled car rebuilt fresh");
            assert_eq!(rpd.cntdest[3], 0);
        }
    }

    /// The fase -8 hold (GameSparker.java:1369-1382): the cantreply
    /// notice holds 150 passes or until a key / mouse press, then
    /// restores the paused frame (the `fleximg` backdrop plus the
    /// `paused` plate) and hands back to the pause screen (fase -7).
    #[test]
    fn fasecantreply_holds_then_restores_the_pause_frame() {
        let mut xt = XtGraphics::new();
        xt.loadpak();
        let mut m = Medium::new();
        let mut cv = Canvas::new(550, 400);
        xt.pauseimage(&mut cv, &mut m); // the fleximg backdrop
        let mut hold = 0i32;
        let mut ctl = Control::new();

        xt.fase = -8;
        for _ in 0..149 {
            fasecantreply(&mut xt, &mut cv, &m, &mut hold, &mut ctl, 0);
        }
        assert_eq!(hold, 149);
        assert_eq!(xt.fase, -8, "still holding");

        fasecantreply(&mut xt, &mut cv, &m, &mut hold, &mut ctl, 0);
        assert_eq!(hold, 150);
        assert_eq!(xt.fase, -7, "the hold ended into the pause screen");

        // the restored frame: the fleximg blit plus the paused plate
        let fp = xt.fleximg.px[0];
        assert_eq!(
            [cv.px[0], cv.px[1], cv.px[2]],
            [
                ((fp >> 16) & 0xFF) as u8,
                ((fp >> 8) & 0xFF) as u8,
                (fp & 0xFF) as u8
            ],
            "the backdrop restored"
        );
        let k = xt
            .paused
            .px
            .iter()
            .position(|&p| p >> 24 == 0xFF)
            .expect("an opaque plate pixel");
        let (px, py) = ((k % xt.paused.w) as i32, (k / xt.paused.w) as i32);
        let pv = xt.paused.px[k];
        let i = ((106 + py) as usize * 550 + (156 + px) as usize) * 3;
        assert_eq!(
            [cv.px[i], cv.px[i + 1], cv.px[i + 2]],
            [
                ((pv >> 16) & 0xFF) as u8,
                ((pv >> 8) & 0xFF) as u8,
                (pv & 0xFF) as u8
            ],
            "the plate redrawn"
        );

        // a key ends the hold early
        xt.fase = -8;
        hold = 0;
        ctl.enter = true;
        fasecantreply(&mut xt, &mut cv, &m, &mut hold, &mut ctl, 0);
        assert_eq!(xt.fase, -7);
        assert!(!ctl.enter);

        // a mouse press too
        xt.fase = -8;
        hold = 0;
        fasecantreply(&mut xt, &mut cv, &m, &mut hold, &mut ctl, 1);
        assert_eq!(xt.fase, -7);
    }

    /// The fase -4 result fade (GameSparker.java:1318-1345): the
    /// `mdness` plate on the first pass, `fleximage` over the live
    /// frame for the shared `var22` counter (grab + re-hue, then five
    /// darkening passes), the `gameov`/`congrd` banner from the second
    /// pass, and the sixth pass closing into fase -5.
    #[test]
    fn faseresult4_fades_into_the_finish_screen() {
        use crate::random::RngSource;

        let mut xt = XtGraphics::new();
        xt.loadpak();
        let mut m = Medium::new();
        xt.snap(1, &m); // mdness is the snapped variant
        let mut cv = Canvas::new(550, 400);
        cv.fill_rect(0, 0, 550, 400, [40, 60, 80]);
        let mut cp = CheckPoints::new();
        cp.stage = 1;
        xt.unlocked = 1;
        xt.winner = false;
        let mut hold = 0i32;
        let mut ctl = Control::new();

        // The flex dither consumes one random() draw per pixel per pass
        // (xt:2289-2376) and refills the walk mid-frame, so no walk
        // preset pins a deep pixel's gate. The TestRng usage rule's own
        // answer for late-class value determinism applies: a Medium
        // subclass override — here the vector source at 0.05, so every
        // dither gate (0.05 as f64 > 0.2) is false and the flex passes
        // preserve the grabbed frame exactly.
        m.rng = Some(RngSource::Vector(vec![0.05f32], 0));

        // pass 1: the mdness plate, no banner yet
        faseresult4(&mut xt, &mut cv, &mut m, &cp, &mut hold, &mut ctl);
        assert_eq!(hold, 1);
        assert_eq!(xt.fase, 111, "the fade is still running");
        let k = xt
            .mdness
            .px
            .iter()
            .position(|&p| p >> 24 == 0xFF)
            .expect("an opaque mdness pixel");
        let (px, py) = ((k % xt.mdness.w) as i32, (k / xt.mdness.w) as i32);
        let mv = xt.mdness.px[k];
        let i = ((330 + py) as usize * 550 + (164 + px) as usize) * 3;
        assert_eq!(
            [cv.px[i], cv.px[i + 1], cv.px[i + 2]],
            [
                ((mv >> 16) & 0xFF) as u8,
                ((mv >> 8) & 0xFF) as u8,
                (mv & 0xFF) as u8
            ],
            "the mdness plate"
        );

        // pass 2 brings the gameov banner
        faseresult4(&mut xt, &mut cv, &mut m, &cp, &mut hold, &mut ctl);
        assert_eq!(hold, 2);
        let k = xt
            .gameov
            .px
            .iter()
            .position(|&p| p >> 24 == 0xFF)
            .expect("an opaque gameov pixel");
        let (px, py) = ((k % xt.gameov.w) as i32, (k / xt.gameov.w) as i32);
        let gv = xt.gameov.px[k];
        let i = ((117 + py) as usize * 550 + (190 + px) as usize) * 3;
        assert_eq!(
            [cv.px[i], cv.px[i + 1], cv.px[i + 2]],
            [
                ((gv >> 16) & 0xFF) as u8,
                ((gv >> 8) & 0xFF) as u8,
                (gv & 0xFF) as u8
            ],
            "the loss banner"
        );

        // passes 3..6 close the fade
        for _ in 0..4 {
            faseresult4(&mut xt, &mut cv, &mut m, &cp, &mut hold, &mut ctl);
        }
        assert_eq!(hold, 6);
        assert_eq!(xt.fase, -5, "the finish screen takes over");

        // the winner-newest branch consumes the press from pass 2 on
        let mut xt = XtGraphics::new();
        xt.loadpak();
        xt.winner = true;
        xt.unlocked = 1;
        let mut m = Medium::new();
        let mut cv = Canvas::new(550, 400);
        cv.fill_rect(0, 0, 550, 400, [40, 60, 80]);
        let mut cp = CheckPoints::new();
        cp.stage = 1;
        let mut hold = 0i32;
        let mut ctl = Control::new();
        ctl.enter = true;
        faseresult4(&mut xt, &mut cv, &mut m, &cp, &mut hold, &mut ctl);
        assert!(ctl.enter, "pass 1 has no banner to swallow the key");
        faseresult4(&mut xt, &mut cv, &mut m, &cp, &mut hold, &mut ctl);
        assert!(!ctl.enter, "the banner pass consumed it");
    }
}
