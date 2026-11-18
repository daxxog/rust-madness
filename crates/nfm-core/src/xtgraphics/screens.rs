// FIDELITY: xtGraphics screen state machines keep the Java per-screen method arity (decompiled XtGraphics.java).
#![allow(clippy::too_many_arguments)]
//! The screen state machines — one method per `xtGraphics` screen, from
//! the loading bar through the stage list, the menu trunk, the pre-race
//! notices, the car select and the result screen. Each body mirrors its
//! decompiled method line-for-line.

use super::*;

impl XtGraphics {
    /// `xtGraphics.loading(Graphics, Applet)` (xt:2940-2966): the loading
    /// screen — black fill, the sign and hello sprites, the F-cell panel
    /// round rects (`fillRoundRect(125,315,300,80,30,70)` in
    /// (198,214,255), (128,167,255) stroke — the exact geometry the
    /// roundrect battery pinned), the loadbar bg, the two centered text
    /// lines, the white text box and the progress bar (`fillRect(162,346,
    /// 26 + (int)(dnload/kbload*200.0F), 10)`).
    ///
    /// The `setFont(new Font("SansSerif", 1, 11))` / `getFontMetrics`
    /// pair is the guidance for both drawcs calls' `size` argument: the
    /// port's glyphs are the shared atlas (`xtfont`), the exact raster that
    /// font produced on the pinned platform. The `Applet.repaint()` tail is
    /// the shell's loop, the caller's.
    pub fn loading(&mut self, cv: &mut Canvas, m: &Medium) {
        cv.fill_rect(0, 0, 550, 400, [0, 0, 0]);
        cv.draw_image(&self.sign, 237, 10);
        cv.draw_image(&self.hello, 0, 80);
        cv.fill_round_rect(125, 315, 300, 80, 30, 70, [198, 214, 255]);
        cv.draw_round_rect(125, 315, 300, 80, 30, 70, [128, 167, 255]);
        cv.draw_image(&self.loadbar, 156, 340);
        self.drawcs(cv, 11, 333, "Loading game, please wait.", [0, 0, 0], 3, m);
        cv.fill_rect(170, 373, 210, 17, [255, 255, 255]);
        // The source builds `"" + (int)((26.0F + (float)dnload /
        // (float)kbload * 200.0F) / 226.0F * 100.0F) + " % loaded    |
        //    " + (kbload - dnload) + " KB remaining"`.
        let pct = ((26.0f32 + self.dnload as f32 / self.kbload as f32 * 200.0f32) / 226.0f32
            * 100.0f32) as i32;
        self.drawcs(
            cv,
            11,
            385,
            &format!(
                "{} % loaded    |    {} KB remaining",
                pct,
                self.kbload - self.dnload
            ),
            [32, 64, 128],
            3,
            m,
        );
        cv.fill_rect(
            162,
            346,
            26 + (self.dnload as f32 / self.kbload as f32 * 200.0f32) as i32,
            10,
            [32, 64, 128],
        );
    }

    /// `xtGraphics.clicknow(Graphics)` (xt:3364-3379): the fase-111 boot
    /// screen — the same 125,315,300,80/30,70 panel as `loading`'s,
    /// painted over whatever the frame carries (no clear — the shell's
    /// black prelude does), with the "Click here to Start" caption
    /// flipping colors through `aflk` each call.
    pub fn clicknow(&mut self, cv: &mut Canvas, m: &Medium) {
        cv.fill_round_rect(125, 315, 300, 80, 30, 70, [198, 214, 255]);
        cv.draw_round_rect(125, 315, 300, 80, 30, 70, [128, 167, 255]);
        if self.aflk {
            self.drawcs(cv, 11, 355, "Click here to Start", [0, 0, 0], 3, m);
            self.aflk = false;
        } else {
            self.drawcs(cv, 11, 355, "Click here to Start", [0, 67, 200], 3, m);
            self.aflk = true;
        }
    }

    /// `xtGraphics.rad(Graphics, int)` (xt:3390-3419): the fase-9
    /// radicalplay splash. The source's `powerup.play()` on the first
    /// frame rides with the audio slice. The `www.radicalplay.com` line
    /// jitters its baseline `(int)(10.0F * m.random())` pixels per call;
    /// the quote line flips y 190/192 and colors through `aflk`.
    /// The two sizes come from the source's `setFont` pair (13 then 11,
    /// xt:3398-3401), as in `loading` (which sets 11).
    pub fn rad(&mut self, cv: &mut Canvas, m: &mut Medium, first: bool) {
        // xt:3392 — the powerup sting on the first frame (no mutes gate).
        if first && let Some(p) = self.powerup.as_mut() {
            p.play();
        }
        cv.fill_rect(0, 0, 550, 400, [0, 0, 0]);
        cv.draw_image(&self.radicalplay, 87, 110);
        let yjitter = (10.0f32 * m.random()) as i32;
        self.drawcs(
            cv,
            13,
            150 + yjitter,
            "www.radicalplay.com",
            [112, 120, 143],
            3,
            m,
        );
        if self.aflk {
            self.drawcs(
                cv,
                11,
                190,
                "And we are never going to find the new unless we get a little crazy...",
                [112, 120, 143],
                3,
                m,
            );
            self.aflk = false;
        } else {
            self.drawcs(
                cv,
                11,
                192,
                "And we are never going to find the new unless we get a little crazy...",
                [150, 150, 150],
                3,
                m,
            );
            self.aflk = true;
        }
        cv.draw_image(&self.rpro, 150, 240);
    }

    /// `xtGraphics.cantgo(Graphics, Control)` (xt:263-299): the
    /// locked-stage screen over the stage-select backdrop. The locked
    /// caption flips black/red through `aflk`; `lockcnt` (armed to 70 by
    /// the stage-select screen, xt:2528) counts down, or the player keys
    /// out, and the shell drops back to fase 1.
    pub fn cantgo(&mut self, cv: &mut Canvas, m: &Medium, ctl: &mut Control) {
        self.pnext = 0;
        cv.draw_image(&self.trackbg, 0, 0);
        self.drawcs(
            cv,
            13,
            110,
            &format!(
                "> You need to complete current stage {} before playing this one...",
                self.unlocked
            ),
            [0, 0, 0],
            3,
            m,
        );
        cv.draw_image(&self.pgate, 96, 150);
        if self.aflk {
            self.drawcs(
                cv,
                13,
                160,
                &format!("[ Stage {} Locked ]", self.unlocked + 1),
                [0, 0, 0],
                3,
                m,
            );
            self.aflk = false;
        } else {
            self.drawcs(
                cv,
                13,
                160,
                &format!("[ Stage {} Locked ]", self.unlocked + 1),
                [255, 0, 0],
                3,
                m,
            );
            self.aflk = true;
        }
        cv.draw_image(&self.select, 201, 45);
        cv.draw_image(&self.bl, 0, 0);
        cv.draw_image(&self.bt, 0, 0);
        cv.draw_image(&self.br, 509, 0);
        cv.draw_image(&self.bb, 0, 357);
        cv.draw_image(&self.back[self.pback as usize], 245, 320);
        self.drawcs(
            cv,
            11,
            396,
            "Use keyboard Arrows and press Enter to continue",
            [0, 0, 0],
            3,
            m,
        );
        self.lockcnt += -1;
        if self.lockcnt == 0 || ctl.enter || ctl.handb || ctl.left {
            ctl.left = false;
            ctl.handb = false;
            ctl.enter = false;
            self.fase = 1;
        }
    }

    /// `xtGraphics.loadingstage(int, Graphics)` (xt:302-317): the
    /// stage-load notice over the stage-select backdrop. The source's
    /// `cars.stop()` / `stages.stop()` open it (xt:303-304).
    pub fn loadingstage(&mut self, stage: i32, cv: &mut Canvas, m: &Medium) {
        if let Some(c) = self.cars.as_mut() {
            c.stop();
        }
        if let Some(s) = self.stages.as_mut() {
            s.stop();
        }
        cv.draw_image(&self.trackbg, 0, 0);
        self.drawcs(
            cv,
            13,
            180,
            &format!("Loading Stage {}, please wait...", stage),
            [0, 0, 0],
            3,
            m,
        );
        cv.draw_image(&self.select, 201, 45);
        cv.draw_image(&self.bl, 0, 0);
        cv.draw_image(&self.bt, 0, 0);
        cv.draw_image(&self.br, 509, 0);
        cv.draw_image(&self.bb, 0, 357);
        self.drawcs(
            cv,
            11,
            396,
            "Use keyboard Arrows and press Enter to continue",
            [0, 0, 0],
            3,
            m,
        );
        // the Applet.repaint() tail is the shell's loop.
    }

    /// `xtGraphics.stageselect(Graphics, CheckPoints, Control)`
    /// (xt:2485-2539): the stage-list frame the fase-1 flyover paints
    /// over the scenery — rails + `select` header, the `back`/`next`
    /// arrows hidden at the list ends, the `contin1` start slot, the
    /// white stage title / "Final Party Stage" caption (masked text
    /// band), the "| name |" line in the stage-name dark blue and the
    /// key hint. The `stages` jingle plays under the screen (xt:2486);
    /// enter/handb start the music load (fase 5), clear `m.trk` and stop
    /// it; right steps to the next stage (fase 2 re-load) unless
    /// the current stage is the newest unlocked — then fase 4's lock
    /// screen arms with `lockcnt = 70`; left steps back (fase 2).
    pub fn stageselect(
        &mut self,
        cv: &mut Canvas,
        cp: &mut CheckPoints,
        m: &mut Medium,
        ctl: &mut Control,
    ) {
        if let Some(s) = self.stages.as_mut() {
            s.play();
        }
        cv.draw_image(&self.bl, 0, 0);
        cv.draw_image(&self.bt, 0, 0);
        cv.draw_image(&self.br, 509, 0);
        cv.draw_image(&self.bb, 0, 357);
        cv.draw_image(&self.select, 201, 45);
        if cp.stage != 1 {
            cv.draw_image(&self.back[self.pback as usize], 50, 110);
        }
        if cp.stage != 11 {
            cv.draw_image(&self.next[self.pnext as usize], 440, 110);
        }
        cv.draw_image(&self.contin1[self.pcontin as usize], 232, 170);
        if cp.stage != 11 {
            self.drawcs(
                cv,
                13,
                110,
                &format!("Stage {}  >", cp.stage),
                [255, 255, 255],
                3,
                m,
            );
        } else {
            self.drawcs(cv, 13, 110, "Final Party Stage  >", [255, 255, 255], 3, m);
        }
        self.drawcs(cv, 13, 130, &format!("| {} |", cp.name), [32, 48, 98], 3, m);
        self.drawcs(
            cv,
            11,
            396,
            "Use keyboard Arrows and press Enter to continue",
            [0, 0, 0],
            3,
            m,
        );
        if ctl.handb || ctl.enter {
            self.fase = 5;
            m.trk = false;
            ctl.handb = false;
            ctl.enter = false;
            if let Some(s) = self.stages.as_mut() {
                s.stop();
            }
        }
        if ctl.right && cp.stage != 11 {
            if cp.stage != self.unlocked {
                cp.stage += 1;
                self.fase = 2;
                ctl.right = false;
            } else {
                self.fase = 4;
                self.lockcnt = 70;
                ctl.right = false;
            }
        }
        if ctl.left && cp.stage != 1 {
            cp.stage += -1;
            self.fase = 2;
            ctl.left = false;
        }
    }

    /// `xtGraphics.inst(Graphics, Control)` (xt:320-369): the instruction
    /// pages — three full-screen gif pages (`inst1..inst3`) on the
    /// (214,218,252) paper, page 1/2 pending the `next` arrow, page 3
    /// waiting on `contin2`, all sequenced by the `flipo` machine
    /// (0→1 paint, 2→3 paint, 4→5 paint; enter flips to the next
    /// pending state). The last enter returns to `oldfase`; from the
    /// in-race pause that is fase -7 and the flex frame redraws.
    pub fn inst(&mut self, cv: &mut Canvas, ctl: &mut Control) {
        if self.flipo == 0 {
            cv.fill_rect(0, 0, 550, 400, [214, 218, 252]);
            cv.draw_image(&self.inst1, 0, 0);
            self.flipo = 1;
        }
        if self.flipo == 2 {
            cv.fill_rect(0, 0, 550, 400, [214, 218, 252]);
            cv.draw_image(&self.inst2, 0, 0);
            self.flipo = 3;
        }
        if self.flipo == 4 {
            cv.fill_rect(0, 0, 550, 400, [214, 218, 252]);
            cv.draw_image(&self.inst3, 0, 0);
            self.flipo = 5;
        }
        if self.flipo == 1 || self.flipo == 3 {
            cv.draw_image(&self.next[self.pnext as usize], 460, 370);
        }
        if self.flipo == 5 {
            cv.draw_image(&self.contin2[self.pcontin as usize], 230, 370);
        }
        if ctl.enter {
            if self.flipo == 1 {
                self.flipo = 2;
            }
            if self.flipo == 3 {
                self.flipo = 4;
            }
            if self.flipo == 5 {
                self.flipo = 0;
                self.fase = self.oldfase;
                if self.fase == -7 {
                    cv.draw_image(&self.fleximg, 0, 0);
                }
            }
            ctl.enter = false;
        }
    }

    /// `xtGraphics.credits(Graphics, Control)` (xt:1452-1513): the credits
    /// film strip. flipo 0 (the powerup sting — audio slice), 1 the rad
    /// splash lookalike, 2 the Radicalplay credits page ((bgmain +
    /// `omdness` header) with the full credit roll, 3 the
    /// `www.nfm.com`-style page (`nfmcom` over bgmain). enter/handb/right
    /// advance the strip; flipo 4 wraps to the main menu.
    pub fn credits(&mut self, cv: &mut Canvas, m: &mut Medium, ctl: &mut Control) {
        // xt:1454 — the powerup sting opens the credits (no mutes gate).
        if self.flipo == 0 {
            if let Some(p) = self.powerup.as_mut() {
                p.play();
            }
            self.flipo = 1;
        }
        if self.flipo == 1 {
            cv.fill_rect(0, 0, 550, 400, [0, 0, 0]);
            cv.draw_image(&self.radicalplay, 87, 110);
            let yjitter = (10.0f32 * m.random()) as i32;
            self.drawcs(
                cv,
                13,
                150 + yjitter,
                "www.radicalplay.com",
                [112, 120, 143],
                3,
                m,
            );
            if self.aflk {
                self.drawcs(
                    cv,
                    11,
                    190,
                    "And we are never going to find the new unless we get a little crazy...",
                    [112, 120, 143],
                    3,
                    m,
                );
                self.aflk = false;
            } else {
                self.drawcs(
                    cv,
                    11,
                    192,
                    "And we are never going to find the new unless we get a little crazy...",
                    [150, 150, 150],
                    3,
                    m,
                );
                self.aflk = true;
            }
            cv.draw_image(&self.rpro, 150, 240);
        }
        if self.flipo == 2 {
            cv.draw_image(&self.bgmain, 0, 0);
            cv.draw_image(&self.omdness, 158, 7);
            self.drawcs(cv, 13, 65, "At Radicalplay.com", [0, 0, 0], 3, m);
            self.drawcs(
                cv,
                13,
                100,
                "Cartoon 3D Engine, Game Programming, 3D Models, Graphics and Sound Effects",
                [0, 0, 0],
                3,
                m,
            );
            self.drawcs(cv, 13, 120, "By Omar Waly", [0, 0, 100], 3, m);
            self.drawcs(cv, 13, 180, "Thanks for Game Testing", [0, 0, 0], 3, m);
            self.drawcs(
                cv,
                13,
                200,
                "Khaled Helmy, Ismail Gorilaz,",
                [0, 0, 100],
                3,
                m,
            );
            self.drawcs(
                cv,
                13,
                215,
                "Karim AboSamra, Mahmoud Waly",
                [0, 0, 100],
                3,
                m,
            );
            self.drawcs(cv, 13, 230, "Karim Khadem, Ahmed Ismail", [0, 0, 100], 3, m);
            self.drawcs(
                cv,
                13,
                245,
                "and Mahmoud EzzElDien (Turbo)",
                [0, 0, 100],
                3,
                m,
            );
            self.drawcs(cv, 13, 305, "Music From", [0, 0, 0], 3, m);
            self.drawcs(cv, 13, 325, "www.ModArchive.com", [0, 0, 100], 3, m);
            self.drawcs(
                cv,
                13,
                385,
                "For any comments: Omar@radicalplay.com",
                [0, 0, 0],
                3,
                m,
            );
        }
        if self.flipo == 3 {
            cv.draw_image(&self.bgmain, 0, 0);
            cv.draw_image(&self.nfmcom, 69, 170);
        }
        cv.draw_image(&self.next[self.pnext as usize], 460, 370);
        if ctl.enter || ctl.handb || ctl.right {
            self.flipo += 1;
            if self.flipo == 4 {
                self.flipo = 0;
                self.fase = 10;
            }
            ctl.enter = false;
            ctl.handb = false;
            ctl.right = false;
        }
    }

    /// `xtGraphics.maini(Graphics, Control)` (xt:3160-3272): the main
    /// menu. The first frame paints `bgmain` + the `maini` button plate
    /// (no clear), then the three option round-rects pulse their
    /// selected highlight (mouse-over gives one solid `shaded` tint on
    /// the highlight rect; otherwise `aflk` alternates the blue/tint
    /// pair each call — exactly one flip per frame, only one block
    /// runs). enter/handb routes: New Game (the fresh run goes fase 11
    /// first with oldfase armed to the carselect return; any other run
    /// white-flashes straight to fase -9 → carselect), Instructions
    /// (oldfase = 10 so inst returns to 10), Credits (fase 8).
    pub fn maini(&mut self, cv: &mut Canvas, ctl: &mut Control) {
        if self.flipo == 0 {
            cv.draw_image(&self.bgmain, 0, 0);
            cv.draw_image(&self.maini, 0, 0);
            self.flipo = 1;
        }
        if ctl.up {
            self.opselect += -1;
            if self.opselect == -1 {
                self.opselect = 2;
            }
            ctl.up = false;
        }
        if ctl.down {
            self.opselect += 1;
            if self.opselect == 3 {
                self.opselect = 0;
            }
            ctl.down = false;
        }
        if self.opselect == 0 {
            if self.shaded {
                cv.fill_rect(218, 246, 110, 22, [32, 48, 160]);
                self.aflk = false;
            }
            // the pulse: each call swaps the highlight between the tint
            // pair, exactly one flip per frame (only one block runs)
            let hl: [u8; 3] = if self.aflk {
                self.aflk = false;
                [143, 159, 239]
            } else {
                self.aflk = true;
                [0, 0, 255]
            };
            cv.draw_round_rect(218, 246, 110, 22, 7, 20, hl);
        } else {
            cv.draw_round_rect(218, 246, 110, 22, 7, 20, [0, 0, 0]);
        }
        if self.opselect == 1 {
            if self.shaded {
                cv.fill_rect(174, 275, 196, 22, [32, 48, 160]);
                self.aflk = false;
            }
            let hl: [u8; 3] = if self.aflk {
                self.aflk = false;
                [143, 159, 239]
            } else {
                self.aflk = true;
                [0, 0, 255]
            };
            cv.draw_round_rect(174, 275, 196, 22, 7, 20, hl);
        } else {
            cv.draw_round_rect(174, 275, 196, 22, 7, 20, [0, 0, 0]);
        }
        if self.opselect == 2 {
            if self.shaded {
                cv.fill_rect(230, 306, 85, 22, [32, 48, 160]);
                self.aflk = false;
            }
            let hl: [u8; 3] = if self.aflk {
                self.aflk = false;
                [143, 159, 239]
            } else {
                self.aflk = true;
                [0, 0, 255]
            };
            cv.draw_round_rect(230, 306, 85, 22, 7, 20, hl);
        } else {
            cv.draw_round_rect(230, 306, 85, 22, 7, 20, [0, 0, 0]);
        }
        cv.draw_image(&self.opti, 181, 250);
        if ctl.enter || ctl.handb {
            if self.opselect == 0 {
                if self.unlocked == 1 && self.oldfase == 0 {
                    self.oldfase = 7;
                    self.fase = 11;
                } else {
                    self.fase = -9;
                }
            }
            if self.opselect == 1 {
                self.oldfase = 10;
                self.fase = 11;
            }
            if self.opselect == 2 {
                self.fase = 8;
            }
            self.flipo = 0;
            ctl.enter = false;
            ctl.handb = false;
        }
    }

    /// The stage-load tint shared by `musicomp` (xt:3274-3283) and the
    /// `framer`/`loadmusic` draw bodies (xt:197-228, xt:1182-1213): the two
    /// Java blocks are literally identical, so this one helper serves all
    /// three. Per channel `(int)(230 - 230 * snap / (100 * hipno[stage-1]))`
    /// clamped 0..255 (the f32 chain truncates like the JVM), overridden
    /// by the fixed (255,230,201) on the hipno-0 stages.
    fn stage_tint(&self, stage: i32, m: &Medium) -> [u8; 3] {
        let hip = self.hipno[(stage - 1) as usize];
        let mut c = [0i32; 3];
        for (i, ci) in c.iter_mut().enumerate() {
            // the source's two clamp ifs, exact for ints
            let v = (230.0f32 - 230.0f32 * (m.snap[i] as f32 / (100 * hip) as f32)) as i32;
            *ci = v.clamp(0, 255);
        }
        if hip == 0 {
            c = [255, 230, 201];
        }
        [c[0] as u8, c[1] as u8, c[2] as u8]
    }

    /// `xtGraphics.musicomp(int stage, Graphics, Control)` (xt:3274-3362):
    /// the pre-race stage notice. The fill is the stage tint (`stage_tint`,
    /// shared with the `framer`/`loadmusic` draw bodies whose Java is
    /// literally identical). The `loadingmusic` plate is
    /// the opsnap'd variant `snap()` set; the star button pulses
    /// `pstar` 0↔1 every call after the draw (`ctachm` parks 2 on the
    /// press), and the newest unlocked stage gets its hint line (mode-4
    /// drawcs, snap-sunk). handb/enter start the race (fase 0) and are
    /// consumed; the source's `System.gc()` rides with the shell.
    pub fn musicomp(&mut self, stage: i32, cv: &mut Canvas, m: &Medium, ctl: &mut Control) {
        cv.fill_rect(0, 0, 550, 400, self.stage_tint(stage, m));
        cv.draw_image(&self.loadingmusic, 164, 90);
        self.drawcs(
            cv,
            11,
            250,
            "Loading complete!  press start to begin...",
            [0, 0, 0],
            3,
            m,
        );
        cv.draw_image(&self.star[self.pstar as usize], 234, 280);
        if stage == 10 {
            if self.aflk {
                self.drawcs(
                    cv,
                    11,
                    340,
                    "> Note: Guidance Arrow is disabled in this stage!",
                    [200, 0, 0],
                    3,
                    m,
                );
                self.aflk = false;
            } else {
                self.drawcs(
                    cv,
                    11,
                    340,
                    "> Note: Guidance Arrow is disabled in this stage!",
                    [0, 0, 0],
                    3,
                    m,
                );
                self.aflk = true;
            }
        }
        if stage == self.unlocked {
            if stage == 1 {
                self.drawcs(
                    cv,
                    11,
                    340,
                    "> Don't forget, you must pass in all checkpoints to complete a lap...",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
            if stage == 2 {
                self.drawcs(
                    cv,
                    11,
                    340,
                    "> Don't forget, you need power to be up to race faster...",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
            if stage == 3 {
                self.drawcs(
                    cv, 11,
                    340,
                    "> Hint: its easier to waste the other cars then to finish 1st in this stage...",
                    [100, 100, 100],
                    4,
                    m,
                );
                self.drawcs(
                    cv,
                    11,
                    360,
                    "( Press [A] to make Guidance Arrow point to cars )",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
            if stage == 4 {
                self.drawcs(
                    cv,
                    11,
                    340,
                    "> Remember the better the stunt the better the power you get...",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
            if stage == 5 {
                self.drawcs(
                    cv,
                    11,
                    340,
                    "> Remember the more the power the more faster and powerful your car is...",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
        }
        if self.pstar != 2 {
            self.pstar = if self.pstar == 0 { 1 } else { 0 };
        }
        if ctl.handb || ctl.enter {
            self.fase = 0;
            ctl.handb = false;
            ctl.enter = false;
        }
    }

    /// `xtGraphics.framer(int stage, Graphics)` (xt:196-261): the
    /// stage-load progress screen. The draw body is literally identical to
    /// `loadmusic`'s (xt:1182-1245), so both run through here: the
    /// `stage_tint` fill, the opsnap'd `loadingmusic` plate at (164,90),
    /// the "N KB" + " Please Wait..." mode-3 rows, the stage-10 guidance
    /// note at 300, and the newest-unlocked-stage hint lines (mode 4,
    /// snap-sunk; stage 3 takes the two-line variant).
    pub fn framer(&mut self, stage: i32, cv: &mut Canvas, m: &Medium) {
        cv.fill_rect(0, 0, 550, 400, self.stage_tint(stage, m));
        cv.draw_image(&self.loadingmusic, 164, 90);
        self.drawcs(
            cv,
            11,
            225,
            &format!("{} KB", self.sndsize[(stage - 1) as usize]),
            [0, 0, 0],
            3,
            m,
        );
        self.drawcs(cv, 11, 260, " Please Wait...", [0, 0, 0], 3, m);
        if stage == 10 {
            self.drawcs(
                cv,
                11,
                300,
                "> Note: Guidance Arrow is disabled in this stage!",
                [100, 100, 100],
                4,
                m,
            );
        }
        if stage == self.unlocked {
            if stage == 1 {
                self.drawcs(
                    cv,
                    11,
                    300,
                    "> Don't forget, you must pass in all checkpoints to complete a lap...",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
            if stage == 2 {
                self.drawcs(
                    cv,
                    11,
                    300,
                    "> Don't forget, you need power to be up to race faster...",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
            if stage == 3 {
                self.drawcs(
                    cv, 11,
                    300,
                    "> Hint: its easier to waste the other cars then to finish 1st in this stage...",
                    [100, 100, 100],
                    4,
                    m,
                );
                self.drawcs(
                    cv,
                    11,
                    320,
                    "( Press [A] to make Guidance Arrow point to cars )",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
            if stage == 4 {
                self.drawcs(
                    cv,
                    11,
                    300,
                    "> Remember the better the stunt the better the power you get...",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
            if stage == 5 {
                self.drawcs(
                    cv,
                    11,
                    300,
                    "> Remember the more the power the more faster and powerful your car is...",
                    [100, 100, 100],
                    4,
                    m,
                );
            }
        }
    }

    /// `xtGraphics.loadmusic(int stage, int hold, Graphics)` (xt:1181-1340):
    /// the draw body duplicates `framer`'s line-for-line, so this runs
    /// `framer` first. Then the per-stage `RadicalMod` construction
    /// (xt:1250-1321, one `(gain, rate, bpm)` tuple per stage):
    /// `stracks[stage - 1]` is (re)built while unarmed and `loadedt`
    /// records only a successful mix (the source's
    /// `if (this.stracks[i].stream != null)`). `hold` is the source's
    /// `var2` (the cookie-less default is 80, so the else branch is the
    /// live path): 0 plays the track and lands fase 6, anything else
    /// parks fase 176; then `pcontin` clears and both mutes drop. The
    /// source's `setCursor`/`repaint` calls ride with the shell.
    pub fn loadmusic(&mut self, stage: i32, hold: i32, cv: &mut Canvas, m: &Medium) {
        /// `(gain, rate, bpm)` per stage (xt:1250-1321) in source order —
        /// `RadicalMod(path, gain, rate, bpm)`.
        const STAGE_TUNES: [(i32, i32, i32); 11] = [
            (350, 8400, 135),
            (370, 9000, 145),
            (350, 8500, 145),
            (300, 7500, 125),
            (250, 7900, 125),
            (760, 7900, 125),
            (300, 7500, 125),
            (400, 7900, 125),
            (300, 7900, 125),
            (550, 8100, 145),
            (550, 9000, 145),
        ];
        self.framer(stage, cv, m);
        let idx = (stage - 1) as usize;
        if !self.loadedt[idx] {
            let (gain, rate, bpm) = STAGE_TUNES[idx];
            self.stracks[idx] = self
                .load_track(&format!("music/stage{stage}.zipo"))
                .map(|b| RadicalMod::new(&b, gain, rate, bpm));
            if self.stracks[idx]
                .as_ref()
                .is_some_and(|r| r.stream.is_some())
            {
                self.loadedt[idx] = true;
            }
        }
        if hold == 0 {
            if let Some(r) = self.stracks[idx].as_mut() {
                r.play();
            }
            self.fase = 6;
        } else {
            self.fase = 176;
        }
        self.pcontin = 0;
        self.mutem = false;
        self.mutes = false;
    }

    /// `xtGraphics.trackbg(Graphics)` (xt:2479-2483): the stage-list
    /// backdrop blit. The gate reads the `stages` RadicalMod position:
    /// with no drain running (see `radicalmod` docs) `posit()` sits at
    /// the full mixed length once loaded (298530 for the stages jingle),
    /// so the posit branch trips whenever the track is present — a
    /// loaded `stages` draws even on restarts (`nrnd = 2`). A `None`
    /// stages posits 0 and the gate collapses to the old `m.nrnd <= 1`.
    pub fn trackbg(&mut self, cv: &mut Canvas, m: &Medium) {
        let posit = self.stages.as_ref().map_or(0, |s| s.posit());
        if posit > 240000 || m.nrnd <= 1 {
            cv.draw_image(&self.trackbg, 0, 0);
        }
    }

    /// `xtGraphics.cantreply(Graphics)` (xt:3450-3456): the
    /// not-enough-replay notice pill.
    pub fn cantreply(&mut self, cv: &mut Canvas, m: &Medium) {
        cv.fill_round_rect(75, 171, 400, 23, 7, 20, [64, 143, 223]);
        cv.draw_round_rect(75, 171, 400, 23, 7, 20, [0, 89, 223]);
        self.drawcs(
            cv,
            11,
            187,
            "Sorry not enough replay data to play available, please try again later.",
            [255, 255, 255],
            1,
            m,
        );
    }

    /// `xtGraphics.replyn(Graphics)` (xt:1007-1015): the "Replay >"
    /// caption that alternates colors per frame (mode-0 drawcs — no
    /// snap lift). Painted each film frame by the replay shell.
    pub fn replyn(&mut self, cv: &mut Canvas, m: &Medium) {
        if self.aflk {
            self.drawcs(cv, 11, 30, "Replay  >", [0, 0, 0], 0, m);
            self.aflk = false;
        } else {
            self.drawcs(cv, 11, 30, "Replay  >", [0, 128, 255], 0, m);
            self.aflk = true;
        }
    }

    /// `xtGraphics.nofocus(Graphics)` (xt:1101-1113): the applet
    /// focus-lost frame — white rails, the gray inner line and the two
    /// caption rows.
    pub fn nofocus(&mut self, cv: &mut Canvas, m: &Medium) {
        cv.fill_rect(0, 0, 550, 20, [255, 255, 255]);
        cv.fill_rect(0, 0, 20, 400, [255, 255, 255]);
        cv.fill_rect(0, 380, 550, 20, [255, 255, 255]);
        cv.fill_rect(530, 0, 20, 400, [255, 255, 255]);
        cv.draw_rect(20, 20, 510, 360, [192, 192, 192]);
        self.drawcs(
            cv,
            11,
            14,
            "Game lost its focus -Click- screen with mouse to continue.",
            [100, 100, 100],
            3,
            m,
        );
        self.drawcs(
            cv,
            11,
            395,
            "Game lost its focus -Click- screen with mouse to continue.",
            [100, 100, 100],
            3,
            m,
        );
    }

    /// `xtGraphics.loadingfailed(int, Graphics, Control)` (xt:3982-4010):
    /// the stage-download failure screen; handb/enter re-arms fase 2.
    pub fn loadingfailed(&mut self, stage: i32, cv: &mut Canvas, m: &Medium, ctl: &mut Control) {
        cv.draw_image(&self.trackbg, 0, 0);
        cv.draw_image(&self.select, 201, 45);
        self.drawcs(
            cv,
            13,
            140,
            &format!("Error Loading Stage {}", stage),
            [200, 0, 70],
            3,
            m,
        );
        self.drawcs(
            cv,
            13,
            170,
            "Your internet connection may have been lost...",
            [0, 0, 0],
            3,
            m,
        );
        self.drawcs(cv, 13, 220, "Press Enter to try again.", [0, 0, 0], 3, m);
        cv.draw_image(&self.contin1[self.pcontin as usize], 232, 270);
        cv.draw_image(&self.bl, 0, 0);
        cv.draw_image(&self.bt, 0, 0);
        cv.draw_image(&self.br, 509, 0);
        cv.draw_image(&self.bb, 0, 357);
        if ctl.handb || ctl.enter {
            self.fase = 2;
            ctl.handb = false;
            ctl.enter = false;
        }
    }

    /// `xtGraphics.over(Image, mx, my, x, y)` (xt:287-291): the
    /// image-anchored hit test, +5px slack around the slot. `pub` for the
    /// mirror's pure-helper pins (it is a pure predicate).
    pub fn over(&self, img: &Gif, xm: i32, ym: i32, x: i32, y: i32) -> bool {
        xm > x - 5 && xm < x + img.w as i32 + 5 && ym > y - 5 && ym < y + img.h as i32 + 5
    }

    /// `xtGraphics.overon(x, y, w, h, mx, my)` (xt:1126-1130): the exact
    /// rect hit test (no slack).
    pub fn overon(&self, x: i32, y: i32, w: i32, h: i32, xm: i32, ym: i32) -> bool {
        xm > x && xm < x + w && ym > y && ym < y + h
    }

    /// `xtGraphics.levelhigh(Graphics, int wasted, int whenwasted)`
    /// (xt:744-755): the crash-cam headline — the `gameh` plate at (176,20)
    /// and one mode-0 title row ("You wasted 'em!" for a competitor's
    /// wreck, "Wasted!" when the film's hold sits at the skipped-crash
    /// marker 229, "Stunts!" otherwise), then the continue prompt. Called
    /// by `fasewasted3` every non-skipped film frame
    /// (GameSparker.java:1266).
    pub fn levelhigh(&mut self, cv: &mut Canvas, wasted: i32, whenwasted: i32, m: &Medium) {
        cv.draw_image(&self.gameh, 176, 20);
        if wasted != 0 {
            self.drawcs(cv, 11, 60, "You wasted 'em!", [16, 48, 96], 0, m);
        } else if whenwasted == 229 {
            self.drawcs(cv, 11, 60, "Wasted!", [16, 48, 96], 0, m);
        } else {
            self.drawcs(cv, 11, 60, "Stunts!", [16, 48, 96], 0, m);
        }
        self.drawcs(
            cv,
            11,
            380,
            "Press  [ Enter ]  to continue",
            [0, 0, 0],
            0,
            m,
        );
    }

    /// `xtGraphics.carselect(Control, ContO[], Madness, Graphics)`
    /// (xt:3557-3705): the car-select screen. The `cars` jingle, then
    /// `carsbg` + `selectcar` header; the camera drops into the
    /// car-select rig (`m.crs`, x/y/z -275/-500/-50, ground 470) and
    /// the selected contour renders once. In the idle state
    /// (`flipo == 0`) the name pulses through `aflk`, the contour is
    /// parked at its next idle pose (z 950, the gateless y, xz spin
    /// +5, wzy wobble), the back/next arrows cap the list ends, and
    /// either the lock panel (stage requirement `(sc-4)*2`,
    /// `gatey` sliding the pgate up) or the five stat bars (SWITS/
    /// ACELF·GRIP/DISHANDLE/AIRC·AIRS/OUTDAM·moment/POWERLOSS formulas
    /// with the source's clamps) draw. The slide state (`flipo` 20→1)
    /// lifts the contour 100/frame, swaps `sc[0]` at 10 and drops the
    /// new one in from -1100. right/left arm the slide (list-end
    /// guarded); handb/enter — idle and the car unlocked — releases the
    /// camera rig and starts the stage load (fase 2).
    ///
    /// `car_m` is the CONTOUR's own `Medium` (ContO.java:64 holds one;
    /// `d()` reads it at xt:3557-3704). The in-game loadbase pool is built
    /// with the global `Medium`, so every production caller passes `None`
    /// and the contour draws through `m` — behaviour-identical. The pixel
    /// fixture's `loadCars` builds its roster with a SEPARATE fresh Medium
    /// (`PixelSuite.java:1100`), and `freshCars` shares it across the copy
    /// ctors (`ContO.java:422`), so the mirror supplies that one; the
    /// committed carselect rows are drawn with the default camera, not the
    /// car-select rig (B12's rig description covers the chrome only).
    pub fn carselect(
        &mut self,
        ctl: &mut Control,
        car: &mut Conto,
        mad: &Madness,
        cv: &mut Canvas,
        m: &mut Medium,
        t: &Trackers,
        car_m: Option<&mut Medium>,
    ) {
        // xt:3358 — the `cars` jingle opens the screen.
        if let Some(c) = self.cars.as_mut() {
            c.play();
        }
        cv.draw_image(&self.carsbg, 0, 0);
        cv.draw_image(&self.selectcar, 184, 65);
        m.crs = true;
        m.x = -275;
        m.y = -500;
        m.z = -50;
        m.xz = 0;
        m.zy = 10;
        m.ground = 470;
        match car_m {
            Some(cm) => car.d(cv, cm, t),
            None => car.d(cv, m, t),
        }
        if self.flipo == 0 {
            let sc = self.sc[0] as usize;
            if self.aflk {
                self.drawcs(cv, 13, 130, self.names[sc], [130, 130, 255], 3, m);
                self.aflk = false;
            } else {
                self.drawcs(cv, 13, 130, self.names[sc], [130, 215, 255], 3, m);
                self.aflk = true;
            }
            car.z = 950;
            car.y = -34 - car.grat;
            car.x = 0;
            car.xz += 5;
            car.zy = 0;
            car.wzy -= 10;
            if car.wzy < -45 {
                car.wzy += 45;
            }
            if self.sc[0] != 0 {
                cv.draw_image(&self.back[self.pback as usize], 23, 270);
            }
            if self.sc[0] != 9 {
                cv.draw_image(&self.next[self.pnext as usize], 467, 270);
            }
            if (self.sc[0] - 4) * 2 >= self.unlocked {
                cv.draw_image(&self.pgate, 96, 145 - self.gatey);
                if self.gatey != 0 {
                    self.gatey -= 100;
                }
                self.drawcs(cv, 13, 320, "[ Car Locked ]", [224, 63, 63], 3, m);
                self.drawcs(
                    cv,
                    13,
                    345,
                    &format!(
                        "This car unlocks when stage {} is completed...",
                        (self.sc[0] - 4) * 2
                    ),
                    [160, 176, 255],
                    3,
                    m,
                );
            } else {
                // The stat panel: direct drawString labels at the
                // `setFont(..., 11)` the source sets first (xt:3607), the
                // statb backing plate, then the six black bars that
                // drain from the right.
                let label = [160u8, 176, 255];
                cv.draw_string("Top Speed:", 33, 313, label, 11);
                cv.draw_image(&self.statb, 97, 307);
                cv.draw_string("Acceleration:", 23, 328, label, 11);
                cv.draw_image(&self.statb, 97, 322);
                cv.draw_string("Handling:", 45, 343, label, 11);
                cv.draw_image(&self.statb, 97, 337);
                cv.draw_string("Aerial Control:", 290, 313, label, 11);
                cv.draw_image(&self.statb, 371, 307);
                cv.draw_string("Strength:", 318, 328, label, 11);
                cv.draw_image(&self.statb, 371, 322);
                cv.draw_string("Power Save:", 300, 343, label, 11);
                cv.draw_image(&self.statb, 371, 337);
                let mut v5 = (SWITS[sc][2] - 220) as f32 / 90.0f32;
                cv.fill_rect(
                    (97.0f32 + 156.0f32 * v5) as i32,
                    307,
                    (156.0f32 * (1.0f32 - v5) + 1.0f32) as i32,
                    7,
                    [0, 0, 0],
                );
                v5 = ACELF[sc][1] * ACELF[sc][0] * ACELF[sc][2] * GRIP[sc] / 7700.0f32;
                cv.fill_rect(
                    (97.0f32 + 156.0f32 * v5) as i32,
                    322,
                    (156.0f32 * (1.0f32 - v5) + 1.0f32) as i32,
                    7,
                    [0, 0, 0],
                );
                v5 = self.dishandle[sc];
                cv.fill_rect(
                    (97.0f32 + 156.0f32 * v5) as i32,
                    337,
                    (156.0f32 * (1.0f32 - v5) + 1.0f32) as i32,
                    7,
                    [0, 0, 0],
                );
                v5 = (AIRC[sc] as f32 * AIRS[sc] + 25.0f32) / 125.0f32;
                if v5 > 1.0f32 {
                    v5 = 1.0f32;
                }
                cv.fill_rect(
                    (371.0f32 + 156.0f32 * v5) as i32,
                    307,
                    (156.0f32 * (1.0f32 - v5) + 1.0f32) as i32,
                    7,
                    [0, 0, 0],
                );
                v5 = OUTDAM[sc] as f32 * mad.moment[sc] / 150.0f32;
                if v5 > 1.0f32 {
                    v5 = 1.0f32;
                }
                cv.fill_rect(
                    (371.0f32 + 156.0f32 * v5) as i32,
                    322,
                    (156.0f32 * (1.0f32 - v5) + 1.0f32) as i32,
                    7,
                    [0, 0, 0],
                );
                v5 = POWERLOSS[sc] as f32 / 5_500_000.0f32;
                cv.fill_rect(
                    (371.0f32 + 156.0f32 * v5) as i32,
                    337,
                    (156.0f32 * (1.0f32 - v5) + 1.0f32) as i32,
                    7,
                    [0, 0, 0],
                );
                cv.draw_image(&self.contin2[self.pcontin as usize], 230, 357);
            }
        } else {
            self.pback = 0;
            self.pnext = 0;
            self.gatey = 300;
            if self.flipo > 10 {
                car.y -= 100;
                if self.nextc {
                    car.zy += 20;
                } else {
                    car.zy -= 20;
                }
            } else {
                if self.flipo == 10 {
                    if self.nextc {
                        self.sc[0] += 1;
                    } else {
                        self.sc[0] += -1;
                    }
                    car.z = 950;
                    car.y = -34 - car.grat - 1100;
                    car.x = 0;
                    car.zy = 0;
                }
                car.y += 100;
            }
            self.flipo += -1;
        }
        self.drawcs(
            cv,
            11,
            396,
            "Use keyboard Arrows and press Enter to continue",
            [112, 151, 208],
            3,
            m,
        );
        if ctl.right {
            ctl.right = false;
            if self.sc[0] != 9 && self.flipo == 0 {
                self.nextc = true;
                self.flipo = 20;
            }
        }
        if ctl.left {
            ctl.left = false;
            if self.sc[0] != 0 && self.flipo == 0 {
                self.nextc = false;
                self.flipo = 20;
            }
        }
        if ctl.handb || ctl.enter {
            if self.flipo == 0 && (self.sc[0] - 4) * 2 < self.unlocked {
                m.crs = false;
                self.fase = 2;
            }
            ctl.handb = false;
            ctl.enter = false;
        }
    }

    /// `xtGraphics.finish(CheckPoints, ContO[], Control, Graphics)`
    /// (xt:2083-2276): the result screen over the frozen `fleximg`
    /// backdrop. Winner on the newest stage (`cp.stage == unlocked`):
    /// `congrd` at (140,30) (the stage-11 finale jitters its x by
    /// `(int)(m.random()*10)`), `pin = 60` with the per-stage podium
    /// overrides (stages 2/4/6/8/10 set `pin = 0`, `sc[0] = 5..9` and
    /// the new-car y 60/40/25/70/80): the "Stage N+1 unlocked!" line
    /// pulses through `aflk`, the podium box (208,240,255 + the
    /// (176,217,240) rails) frames the new car rendered through the
    /// carselect camera rig (ground 2470, z 1500) with the aflk frame
    /// (0,128,255)/(255,0,0), "NAME has been unlocked!", then
    /// "( Game Saved )" at `335 - pin`; stage 11 instead paints the
    /// endgame page ("Woohoooo…", "Your Awesome!" flipping y 150/152
    /// and tint, "RADICAL GAMER", the radicalplay blit, the
    /// radicalplay.com line) and sets `pin = 40`. Winner on an older
    /// stage: `congrd` at (140,117), `pin = 100`, one line. Loser:
    /// `gameov` at (190,117), the "You lost…" line and the
    /// newest-stage hint lines. Tail: `contin2[pcontin]` at
    /// `(230, 350 - pin)`; enter/handb return to the menu (fase 10),
    /// stop the stage track when armed (xt:2265), and the
    /// newest-stage win advances `cp.stage`/`unlocked`.
    pub fn finish(
        &mut self,
        cp: &mut CheckPoints,
        cars: &mut [Conto],
        cv: &mut Canvas,
        m: &mut Medium,
        t: &Trackers,
        ctl: &mut Control,
    ) {
        cv.draw_image(&self.fleximg, 0, 0);
        if self.winner {
            if cp.stage == self.unlocked {
                if cp.stage != 11 {
                    cv.draw_image(&self.congrd, 140, 30);
                    self.drawcs(
                        cv,
                        11,
                        80,
                        &format!("You completed stage {} : {} !", cp.stage, cp.name),
                        [144, 167, 255],
                        3,
                        m,
                    );
                } else {
                    cv.draw_image(&self.congrd, 135 + (m.random() * 10.0f32) as i32, 30);
                }
                self.pin = 60;
                let (var5, var6) = match cp.stage {
                    2 => (5, 60),
                    4 => (6, 40),
                    6 => (7, 25),
                    8 => (8, 70),
                    10 => (9, 80),
                    _ => (0, 0),
                };
                if var5 != 0 {
                    self.pin = 0;
                    self.sc[0] = var5;
                }
                if cp.stage != 11 {
                    let c = if self.aflk {
                        [144, 167, 255]
                    } else {
                        [208, 240, 255]
                    };
                    self.drawcs(
                        cv,
                        13,
                        120 + self.pin,
                        &format!("Stage {} unlocked!", cp.stage + 1),
                        c,
                        3,
                        m,
                    );
                    if var5 != 0 {
                        let c = if self.aflk {
                            [144, 167, 255]
                        } else {
                            [208, 240, 255]
                        };
                        self.drawcs(cv, 13, 150, "And:", c, 3, m);
                        cv.fill_rect(200, 170, 150, 100, [208, 240, 255]);
                        cv.fill_rect(201, 171, 148, 4, [176, 217, 240]);
                        cv.fill_rect(201, 171, 4, 98, [176, 217, 240]);
                        cv.fill_rect(201, 265, 148, 4, [176, 217, 240]);
                        cv.fill_rect(345, 171, 4, 98, [176, 217, 240]);
                        // xtGraphics.java:2163-2168 — the podium draws the
                        // freshly unlocked model: `var2[var5]`, indexed by
                        // the podium slot computed above (not the
                        // pre-finish selection).
                        let car = &mut cars[var5 as usize];
                        car.y = var6;
                        m.crs = true;
                        m.x = -275;
                        m.y = -500;
                        m.z = -50;
                        m.xz = 0;
                        m.zy = 10;
                        m.ground = 2470;
                        car.z = 1500;
                        car.x = 0;
                        car.xz += 5;
                        car.zy = 0;
                        car.wzy -= 10;
                        car.d(cv, m, t);
                        let frame = if self.aflk {
                            [0, 128, 255]
                        } else {
                            [255, 0, 0]
                        };
                        cv.draw_rect(197, 167, 155, 105, frame);
                        let c = if self.aflk {
                            [144, 167, 255]
                        } else {
                            [208, 240, 255]
                        };
                        self.drawcs(
                            cv,
                            13,
                            300,
                            &format!("{} has been unlocked!", self.names[var5 as usize]),
                            c,
                            3,
                            m,
                        );
                    }
                    let c = if self.aflk {
                        [112, 120, 143]
                    } else {
                        [150, 150, 150]
                    };
                    self.drawcs(cv, 11, 335 - self.pin, "( Game Saved )", c, 3, m);
                } else {
                    let c = if self.aflk {
                        [144, 167, 255]
                    } else {
                        [208, 240, 255]
                    };
                    self.drawcs(cv, 13, 120, "Woohoooo you finished the game!!!", c, 3, m);
                    if self.aflk {
                        self.drawcs(cv, 13, 150, "Your Awesome!", [144, 167, 255], 3, m);
                    } else {
                        self.drawcs(cv, 13, 152, "Your Awesome!", [228, 240, 255], 3, m);
                    }
                    if self.aflk {
                        self.drawcs(
                            cv,
                            13,
                            200,
                            "You're truly a RADICAL GAMER!",
                            [144, 167, 255],
                            3,
                            m,
                        );
                    } else {
                        self.drawcs(
                            cv,
                            13,
                            200,
                            "You're truly a RADICAL GAMER!",
                            [228, 240, 255],
                            3,
                            m,
                        );
                    }
                    cv.draw_image(&self.radicalplay, 95, 205);
                    if self.aflk {
                        self.drawcs(
                            cv,
                            11,
                            270,
                            "Be sure to check Radicalplay.com for more action!",
                            [144, 167, 255],
                            3,
                            m,
                        );
                    } else {
                        self.drawcs(
                            cv,
                            11,
                            270,
                            "Be sure to check Radicalplay.com for more action!",
                            [208, 240, 255],
                            3,
                            m,
                        );
                    }
                    self.pin = 40;
                }
                self.aflk = !self.aflk;
            } else {
                self.pin = 100;
                cv.draw_image(&self.congrd, 140, 117);
                self.drawcs(
                    cv,
                    11,
                    167,
                    &format!("You completed stage {} : {} !", cp.stage, cp.name),
                    [144, 167, 255],
                    3,
                    m,
                );
            }
        } else {
            self.pin = 100;
            cv.draw_image(&self.gameov, 190, 117);
            self.drawcs(
                cv,
                11,
                167,
                &format!("You lost at stage {} : {} !", cp.stage, cp.name),
                [144, 167, 255],
                3,
                m,
            );
            if cp.stage == self.unlocked {
                if cp.stage == 1 {
                    self.drawcs(
                        cv,
                        11,
                        205,
                        "> Don't forget, you must pass in all checkpoints to complete a lap...",
                        [128, 128, 128],
                        3,
                        m,
                    );
                }
                if cp.stage == 2 {
                    self.drawcs(
                        cv,
                        11,
                        205,
                        "> Don't forget, you need power to be up to race faster...",
                        [128, 128, 128],
                        3,
                        m,
                    );
                }
                if cp.stage == 3 {
                    self.drawcs(cv, 11, 205, "> Hint: its easier to waste the other cars then to finish 1st in this stage...", [128, 128, 128], 3, m);
                    self.drawcs(
                        cv,
                        11,
                        220,
                        "( Press [A] to make Guidance Arrow point to cars )",
                        [128, 128, 128],
                        3,
                        m,
                    );
                }
                if cp.stage == 4 {
                    self.drawcs(
                        cv,
                        11,
                        205,
                        "> Remember the better the stunt the better the power you get...",
                        [128, 128, 128],
                        3,
                        m,
                    );
                }
                if cp.stage == 5 {
                    self.drawcs(
                        cv,
                        11,
                        205,
                        "> Remember the more the power the more faster and powerful your car is...",
                        [128, 128, 128],
                        3,
                        m,
                    );
                }
            }
        }
        cv.draw_image(&self.contin2[self.pcontin as usize], 230, 350 - self.pin);
        if ctl.enter || ctl.handb {
            self.fase = 10;
            if self.loadedt[(cp.stage - 1) as usize]
                && let Some(r) = self.stracks[(cp.stage - 1) as usize].as_mut()
            {
                r.stop();
            }
            if cp.stage == self.unlocked && self.winner && self.unlocked != 11 {
                cp.stage += 1;
                self.unlocked += 1;
            }
            ctl.enter = false;
            ctl.handb = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::md5;
    use crate::xtgraphics::testutil::{
        dump_masked, dump_masked_rects, hex, loaded_xt, pattern_px, rect_ink,
    };

    /// One `loadbase` model as a base (the shared model rig).
    fn base_for(slot: usize) -> crate::conto::Base {
        let raw = std::fs::read("../../assets/graphics/models.zipo").expect("models.zipo");
        let z = crate::zipo::Zipo::read(&raw).expect("zipo");
        for e in &z.entries {
            if crate::stage::model_slot_for_entry(&e.name) == Some(slot) {
                let model = crate::model::Model::parse_rad(&e.name, &e.data).expect("model");
                let mut w = crate::wheels::Wheels::new();
                return crate::conto::Base::new(&model, &Medium::new(), &mut w);
            }
        }
        panic!("no model for slot {slot}");
    }

    /// The loadbase model rig for the carselect tests: slot 4 (`nimi`),
    /// the sc[0] the JDK capture used, as a live contour.
    fn car_rig() -> (XtGraphics, Conto, Medium, Trackers, Madness) {
        let base = base_for(4);
        let mut xt = XtGraphics::new();
        xt.loadpak();
        xt.unlocked = 1;
        xt.sc[0] = 4;
        let m = Medium::new();
        let mut t = Trackers::new();
        let car = Conto::place(&base, &m, &mut t, 0, 0, 0, 0);
        let mad = Madness::new(0);
        (xt, car, m, t, mad)
    }

    #[allow(clippy::type_complexity)]
    fn finish_rig(
        winner: bool,
        unlocked: i32,
        stage: i32,
    ) -> (
        XtGraphics,
        CheckPoints,
        Vec<Conto>,
        Canvas,
        Medium,
        Trackers,
        Control,
    ) {
        let mut xt = XtGraphics::new();
        xt.loadpak();
        xt.winner = winner;
        xt.unlocked = unlocked;
        xt.fleximg = Gif {
            w: 550,
            h: 400,
            px: pattern_px(),
        };
        let mut cp = CheckPoints::new();
        cp.stage = stage;
        cp.name = "test stage".to_string();
        let m = Medium::new();
        let mut t = Trackers::new();
        // the 10-slot model row the shell hands `finish` (GameSparker's
        // `ContO[]` pool); the podium indexes the freshly unlocked slot.
        let cars: Vec<Conto> = (0..10)
            .map(|i| Conto::place(&base_for(i), &m, &mut t, 0, 0, 0, 0))
            .collect();
        let cv = Canvas::new(550, 400);
        let ctl = Control::new();
        (xt, cp, cars, cv, m, t, ctl)
    }

    /// The loading screen (`xtGraphics.loading`, xt:2940-2966) at the
    /// `dnload` 91 midpoint: the JDK capture (scratch `ClickRadGt`-era
    /// local JDK 8u40) masked rows 324..=338 and 376..=390 for its own
    /// platform font; those bands still contain the atlas rows at 11pt.
    /// The 0 KB head of the bar is now the full-frame mirror row
    /// `screen_loading_start_0kb` (and 485 the `_final_95pct` row), which
    /// covers this capture's dnload-0 block; 91 is not Java-pinned and
    /// stays inline (folding rule).
    #[test]
    fn loading_screen_matches_the_jdk_outside_the_text() {
        let mut xt = loaded_xt();
        assert!(
            xt.sign.w > 0 && xt.hello.w > 0 && xt.loadbar.w > 0,
            "loadpak got the loose gifs"
        );
        let m = Medium::new();
        xt.dnload = 91;
        let mut cv = Canvas::new(550, 400);
        xt.loading(&mut cv, &m);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[(324, 338), (376, 390)]))),
            "0f35d1f24e03140c6d0ccf11ac186a0d",
            "dnload 91: masked band md5"
        );
    }

    /// The loading-screen progress line's f32 chain:
    /// `(int)((26.0F + (float)dnload / (float)kbload * 200.0F) /
    /// 226.0F * 100.0F)`, and the bar width `26 +
    /// (int)(dnload/kbload*200.0F)` (26 @ 0 and 61 @ 91 = the widths the
    /// JDK captures pinned).
    #[test]
    fn loading_progress_math_and_bar_width() {
        let mut xt = XtGraphics::new();
        xt.kbload = 514;

        let pct0 = ((26.0f32 + 0.0f32 / 514.0f32 * 200.0f32) / 226.0f32 * 100.0f32) as i32;
        assert_eq!(pct0, 11, "the percent at dnload 0");
        assert_eq!(xt.kbload - xt.dnload, 514, "the KB remaining line");

        let w91 = 26 + (91.0f32 / 514.0f32 * 200.0f32) as i32;
        assert_eq!(w91, 61, "the bar at dnload 91 (the JDK's captured width)");

        // The full state: (26 + 200)/226*100 nets exactly 100 through the
        // f32 chain, KB remaining 0.
        xt.dnload = 514;
        let pct = ((26.0f32 + 514.0f32 / 514.0f32 * 200.0f32) / 226.0f32 * 100.0f32) as i32;
        assert_eq!(pct, 100, "the percent at full load");
        assert_eq!(xt.kbload - xt.dnload, 0);
    }

    // ────────────── the boot screens slice ──────────────

    /// `clicknow` (xt:3364-3379) over the black prelude — the same F-cell
    /// panel, with the caption's colors flipping through `aflk`. The JDK
    /// 1.8.0_40 capture (scratch `ClickRadGt`) masks rows 344..=362 (its
    /// platform font's caption band; the atlas 11pt caption fits there);
    /// the panel and the prelude carry the pin. The flip consumes the fresh rig's false
    /// state on the first call, and the masked frame is branch-stable
    /// since both captions live inside the band.
    ///
    /// This stays inline (the mirror's two `screen_clicknow_*` rows are
    /// unlanded: the Java fixture draws the caption with a second fresh
    /// Graphics' DEFAULT 12pt font, which the committed 13/11 atlas cannot
    /// raster — C3a report, fixture-vs-game conflict). The masked panel
    /// fact here is the coverage until that is resolved.
    #[test]
    fn clicknow_matches_the_jdk_outside_the_caption() {
        let mut xt = loaded_xt();
        let mut cv = Canvas::new(550, 400);
        let m = Medium::new();
        cv.fill_rect(0, 0, 550, 400, [0, 0, 0]); // the run() prelude
        xt.clicknow(&mut cv, &m);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[(344, 362)]))),
            "75aa0d76d1bb5c2aab244ff88a3f89d4",
            "clicknow: masked caption band"
        );
        assert!(xt.aflk, "the first call consumed the false state");

        // the flipped frame re-renders identically outside the band
        cv.fill_rect(0, 0, 550, 400, [0, 0, 0]);
        xt.clicknow(&mut cv, &m);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[(344, 362)]))),
            "75aa0d76d1bb5c2aab244ff88a3f89d4",
            "the band holds both caption colors — the frame outside is stable"
        );
        assert!(!xt.aflk, "the flip alternates per call");
    }

    // `rad`'s masked capture is folded into the full-frame rows
    // `screen_rad_tick{0,1}_aflk_{false,true}` (which include both text
    // rows and the m.random() baseline), plus the
    // `screen_rad_tick_paint_neutral` structural pin in the mirror.

    // ────────────── the menu trunk slice ──────────────

    /// The two menu-adjacent captures that the Java pixel suite does NOT
    /// pin (`screens()` has no `loadingstage`/`cantreply` row), so they
    /// stay inline per the folding rule. Everything else this test used to
    /// pin — `maini`'s first frame, `inst` page 1, the credits `nfmcom`
    /// page, `cantgo`'s lock frame, `loadingfailed`'s retry frame and
    /// `nofocus` — is now a FULL-frame `screen_*` row in the mirror
    /// (`mirror/pixel.rs`), which is strictly stronger (the rows include
    /// the text the masks dropped), so those sub-pins were removed.
    ///
    /// `loadingstage`/`cantreply` are JDK 1.8.0_40 captures (scratch
    /// `MenuGt`, sprites through the pak bytes like `loadimage`); they are
    /// not Java-suite-pinned and are ledgered for a future Java test.
    #[test]
    fn menu_screens_match_the_jdk() {
        // loadingstage — the stage-load notice frame
        let mut xt = loaded_xt();
        let mut cv = Canvas::new(550, 400);
        let m = Medium::new();
        xt.loadingstage(1, &mut cv, &m);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[(170, 184), (387, 400)]))),
            "65cda60007ee704abe846e78a704a465",
            "loadingstage: base + header"
        );

        // cantreply — the notice pill
        let mut xt = loaded_xt();
        let mut cv = Canvas::new(550, 400);
        let m = Medium::new();
        xt.cantreply(&mut cv, &m);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[(178, 191)]))),
            "737ea6fb61834d0e03940a5f56d06b2c",
            "cantreply: the pill"
        );
    }

    /// `maini`'s key machine: up/down move `opselect` with the source's
    /// wrap points, and enter routes by selection — New Game fresh-run
    /// (unlocked 1, oldfase 0) goes to the instructions first with
    /// oldfase armed to the carselect return; a later run white-flashes
    /// (fase -9); instructions arms oldfase 10; credits is fase 8.
    #[test]
    fn maini_keys_and_enter_route_the_menu() {
        let mut xt = loaded_xt();
        let mut cv = Canvas::new(550, 400);
        let mut ctl = Control::new();

        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.flipo, 1);
        assert_eq!(xt.opselect, 1, "Java's field default");

        ctl.up = true;
        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.opselect, 0, "up from 1");
        assert!(!ctl.up, "the key consumed");

        ctl.up = true;
        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.opselect, 2, "up wraps past -1 to 2");

        ctl.down = true;
        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.opselect, 0, "down wraps past 3 to 0");

        ctl.down = true;
        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.opselect, 1);

        ctl.down = true;
        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.opselect, 2);

        // the highlight pulse flips once per frame
        let aflk = xt.aflk;
        xt.maini(&mut cv, &mut ctl);
        assert_ne!(xt.aflk, aflk, "one flip per call");

        // enter on New Game, fresh run → instructions first
        xt.opselect = 0;
        xt.unlocked = 1;
        xt.oldfase = 0;
        ctl.enter = true;
        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.oldfase, 7, "the carselect return armed");
        assert_eq!(xt.fase, 11, "via the instruction pages");
        assert_eq!(xt.flipo, 0, "the menu plate disarmed");
        assert!(!ctl.enter);

        // a later run (unlocked > 1) white-flashes straight to carselect
        xt.opselect = 0;
        xt.unlocked = 2;
        ctl.handb = true;
        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.fase, -9);
        assert!(!ctl.handb);

        // instructions arms oldfase 10
        xt.opselect = 1;
        ctl.enter = true;
        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.oldfase, 10);
        assert_eq!(xt.fase, 11);

        // credits
        xt.opselect = 2;
        ctl.enter = true;
        xt.maini(&mut cv, &mut ctl);
        assert_eq!(xt.fase, 8);
    }

    /// `inst`'s flipo machine: 0→1 (paint), enter 1→2→3 (paint), enter
    /// 3→4→5 (paint), enter 5→0 and back to `oldfase`. From the in-race
    /// pause (oldfase -7) the return redraws the (empty until the flex
    /// slice) fleximg frame.
    #[test]
    fn inst_pages_cycle_and_return_to_oldfase() {
        let mut xt = loaded_xt();
        let mut cv = Canvas::new(550, 400);
        let mut ctl = Control::new();
        xt.oldfase = 10;

        xt.inst(&mut cv, &mut ctl);
        assert_eq!(xt.flipo, 1);
        ctl.enter = true;
        xt.inst(&mut cv, &mut ctl);
        assert_eq!(xt.flipo, 2);
        assert!(!ctl.enter);
        xt.inst(&mut cv, &mut ctl);
        assert_eq!(xt.flipo, 3, "the page-2 frame armed page 3");
        ctl.enter = true;
        xt.inst(&mut cv, &mut ctl);
        assert_eq!(xt.flipo, 4);
        xt.inst(&mut cv, &mut ctl);
        assert_eq!(xt.flipo, 5);
        ctl.enter = true;
        xt.inst(&mut cv, &mut ctl);
        assert_eq!(xt.flipo, 0);
        assert_eq!(xt.fase, 10, "returned to the parent screen");

        // the in-race pause return
        xt.oldfase = -7;
        xt.flipo = 5;
        ctl.enter = true;
        xt.inst(&mut cv, &mut ctl);
        assert_eq!(xt.fase, -7);
        assert_eq!(xt.flipo, 0);
    }

    /// `credits`' flipo strip: enter advances through all four states and
    /// wraps to the main menu.
    #[test]
    fn credits_enters_cycle_to_the_menu() {
        let mut xt = loaded_xt();
        let mut cv = Canvas::new(550, 400);
        let mut m = Medium::new();
        let mut ctl = Control::new();

        xt.credits(&mut cv, &mut m, &mut ctl);
        assert_eq!(xt.flipo, 1, "the sting frame");
        ctl.enter = true;
        xt.credits(&mut cv, &mut m, &mut ctl);
        assert_eq!(xt.flipo, 2);
        ctl.enter = true;
        xt.credits(&mut cv, &mut m, &mut ctl);
        assert_eq!(xt.flipo, 3);
        ctl.handb = true;
        xt.credits(&mut cv, &mut m, &mut ctl);
        assert_eq!(xt.flipo, 0);
        assert_eq!(xt.fase, 10, "back to the main menu");
        assert!(!ctl.handb);
        // right advances too (the same three-key gate); the powerup step
        // and the tail increment land in the same call, so 0 → 2
        ctl.right = true;
        xt.credits(&mut cv, &mut m, &mut ctl);
        assert_eq!(xt.flipo, 2);
        assert!(!ctl.right);
    }

    /// `cantgo`'s lock countdown: the stage-select screen arms lockcnt to
    /// 70; the screen counts it down and drops to fase 1 at zero, or the
    /// player keys out earlier (enter/handb/left, consumed).
    #[test]
    fn cantgo_counts_down_or_keys_out() {
        let mut xt = loaded_xt();
        let mut cv = Canvas::new(550, 400);
        let m = Medium::new();
        let mut ctl = Control::new();

        xt.lockcnt = 3;
        xt.cantgo(&mut cv, &m, &mut ctl);
        assert_eq!(xt.lockcnt, 2);
        assert_eq!(xt.fase, 111, "still locked in");
        xt.cantgo(&mut cv, &m, &mut ctl);
        assert_eq!(xt.lockcnt, 1);
        xt.cantgo(&mut cv, &m, &mut ctl);
        assert_eq!(xt.lockcnt, 0);
        assert_eq!(xt.fase, 1, "the countdown dropped out");

        // the key path consumes the press
        xt.fase = 4;
        xt.lockcnt = 70;
        ctl.handb = true;
        xt.cantgo(&mut cv, &m, &mut ctl);
        assert_eq!(xt.fase, 1);
        assert!(!ctl.handb);
        assert_eq!(xt.lockcnt, 69, "the count still ticked");
    }

    /// `loadingfailed`: handb/enter re-arms the stage load (fase 2) and
    /// both keys are consumed.
    #[test]
    fn loadingfailed_retry_arms_the_loading() {
        let mut xt = loaded_xt();
        let mut cv = Canvas::new(550, 400);
        let m = Medium::new();
        let mut ctl = Control::new();
        xt.fase = 3;

        ctl.enter = true;
        xt.loadingfailed(3, &mut cv, &m, &mut ctl);
        assert_eq!(xt.fase, 2);
        assert!(!ctl.enter);

        xt.fase = 3;
        ctl.handb = true;
        xt.loadingfailed(3, &mut cv, &m, &mut ctl);
        assert_eq!(xt.fase, 2);
        assert!(!ctl.handb);
    }

    /// `musicomp`'s stage tint — the Java f32 chain
    /// `(int)(230 - 230 * snap / (100 * hipno[stage-1]))` clamped 0..255,
    /// with the hipno-0 stages overriding to (255,230,201). Values come
    /// from JDK 1.8.0_40 (scratch `TintGt`); the (25,40,60)/hip-2 row is
    /// the f32-truncation witness (g lands on 184, not the rational 184.0
    /// rounding story).
    #[test]
    fn musicomp_tint_matches_the_jvm() {
        fn tint(snap: i32, hip: i32) -> i32 {
            let v = (230.0f32 - 230.0f32 * (snap as f32 / (100 * hip) as f32)) as i32;
            v.clamp(0, 255)
        }
        assert_eq!(
            [tint(25, 2), tint(40, 2), tint(60, 2)],
            [201, 184, 161],
            "hip 2, snap (25,40,60)"
        );
        assert_eq!(
            [tint(-500, 2), tint(-500, 2), tint(-500, 2)],
            [255, 255, 255],
            "the high clamp"
        );
        assert_eq!([tint(0, 50), tint(0, 50), tint(0, 50)], [230, 230, 230]);
        assert_eq!([tint(200, 2), tint(200, 2), tint(200, 2)], [0, 0, 0]);
        // hip 0 divides by zero: -inf saturates to MIN then clamps to 0,
        // and the caller's override supplies the fixed triple
        assert_eq!(tint(25, 0), 0, "the hipno-0 clamp before the override");
    }

    /// `musicomp`'s composition: the stage-2 tint fills the frame (the
    /// star and loadingmusic plates are the opsnap'd sprites, so their
    /// regions get ink-count checks), the star flips pstar 0→1 after the
    /// draw (parked at 2 by a press), and handb/enter start the race.
    #[test]
    fn musicomp_runs_the_star_machine_and_starts_the_race() {
        let mut xt = loaded_xt();
        let mut m = Medium::new();
        xt.snap(2, &m);
        assert!(
            xt.star[0].w > 0 && xt.loadingmusic.w > 0,
            "the snapped plates"
        );
        m.snap = [25, 40, 60];

        let mut cv = Canvas::new(550, 400);
        let mut ctl = Control::new();
        xt.pstar = 0;
        xt.musicomp(2, &mut cv, &m, &mut ctl);

        // fill corners carry the (201,184,161) tint
        let at = |cv: &Canvas, x: i32, y: i32| {
            let i = (y as usize * cv.w + x as usize) * 3;
            [cv.px[i], cv.px[i + 1], cv.px[i + 2]]
        };
        assert_eq!(at(&cv, 0, 0), [201, 184, 161], "top-left tint");
        assert_eq!(at(&cv, 549, 399), [201, 184, 161], "bottom-right tint");
        assert!(
            rect_ink(&cv, 164, 90, 164 + 300, 90 + 220) > 500,
            "the plate ink"
        );
        assert!(
            rect_ink(&cv, 234, 280, 234 + 60, 280 + 40) > 50,
            "the star ink"
        );
        assert_eq!(xt.pstar, 1, "the pulse flipped after the draw");

        // a press parks the pressed variant (2) and the pulse holds
        xt.pstar = 2;
        xt.musicomp(2, &mut cv, &m, &mut ctl);
        assert_eq!(xt.pstar, 2, "pressed frame held");

        // handb starts the race and consumes the key
        ctl.handb = true;
        xt.musicomp(2, &mut cv, &m, &mut ctl);
        assert_eq!(xt.fase, 0);
        assert!(!ctl.handb);

        // the hipno-0 stages take the fixed tint
        let mut m0 = Medium::new();
        m0.snap = [25, 40, 60];
        let mut cv0 = Canvas::new(550, 400);
        let mut ctl0 = Control::new();
        xt.musicomp(1, &mut cv0, &m0, &mut ctl0); // hipno[0] = 0
        assert_eq!(at(&cv0, 0, 0), [255, 230, 201], "the fixed hipno-0 tint");
    }

    /// `trackbg` (xt:2479-2483) against JDK 1.8.0_40 (scratch `TrackGt`):
    /// a black 550x400 frame plus the stages.jpg blit at (0,0) — the
    /// full frame is byte-exact. With the `stages` jingle loaded from
    /// `loadpak`, `posit()` sits at the full 298530 (> 240000), so the
    /// posit branch draws even with `m.nrnd = 2` (the no-drain headless
    /// semantic); with `stages` parked to `None` the gate collapses to
    /// the old `nrnd <= 1` and the restart skips the blit.
    #[test]
    fn trackbg_matches_the_jdk_on_first_entry() {
        let mut xt = loaded_xt();
        assert!(xt.stages.is_some(), "loadpak built the jingle");
        let mut m = Medium::new();
        m.nrnd = 0;
        let mut cv = Canvas::new(550, 400);
        xt.trackbg(&mut cv, &m);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[]))),
            "a495c9c8f0fa9f7c7dc3b44cca2d633a",
            "stages.jpg over black"
        );

        // the loaded jingle trips the posit branch on restarts too
        m.nrnd = 2;
        let mut cv2 = Canvas::new(550, 400);
        xt.trackbg(&mut cv2, &m);
        assert_eq!(
            hex(&md5(&dump_masked(&cv2, &[]))),
            "a495c9c8f0fa9f7c7dc3b44cca2d633a",
            "loaded stages draws at nrnd 2"
        );

        // without the jingle the old nrnd-only gate holds
        xt.stages = None;
        let mut cv3 = Canvas::new(550, 400);
        xt.trackbg(&mut cv3, &m);
        assert!(cv3.px.iter().all(|&b| b == 0), "the restart gate held");
        m.nrnd = 0;
        let mut cv4 = Canvas::new(550, 400);
        xt.trackbg(&mut cv4, &m);
        assert_eq!(
            hex(&md5(&dump_masked(&cv4, &[]))),
            "a495c9c8f0fa9f7c7dc3b44cca2d633a",
            "first entry still draws"
        );
    }

    /// `framer`'s composition (xt:196-260): the stage-2 tint fills the
    /// frame (the JVM-pinned (201,184,161) for snap (25,40,60)), the
    /// opsnap'd `loadingmusic` plate inks its region, and `loadmusic`
    /// routes the same draw before arming the track: hold 80 (the live
    /// path) lands fase 176 with the flags set, hold 0 lands fase 6.
    #[test]
    fn framer_and_loadmusic_share_the_draw_then_arm() {
        let mut xt = loaded_xt();
        let mut m = Medium::new();
        xt.snap(2, &m);
        m.snap = [25, 40, 60];
        assert!(xt.loadingmusic.w > 0, "the snapped plate");

        let at = |cv: &Canvas, x: i32, y: i32| {
            let i = (y as usize * cv.w + x as usize) * 3;
            [cv.px[i], cv.px[i + 1], cv.px[i + 2]]
        };
        let mut cv = Canvas::new(550, 400);
        xt.framer(2, &mut cv, &m);
        assert_eq!(at(&cv, 0, 0), [201, 184, 161], "top-left tint");
        assert_eq!(at(&cv, 549, 399), [201, 184, 161], "bottom-right tint");
        assert!(
            rect_ink(&cv, 164, 90, 164 + 300, 90 + 220) > 500,
            "the plate ink"
        );

        // the hipno-0 stages take the fixed tint through framer too
        let mut m0 = Medium::new();
        m0.snap = [25, 40, 60];
        let mut cv0 = Canvas::new(550, 400);
        xt.framer(1, &mut cv0, &m0); // hipno[0] = 0
        assert_eq!(at(&cv0, 0, 0), [255, 230, 201], "the fixed hipno-0 tint");

        // the live path: hold 80 arms the track and parks fase 176
        let mut cvl = Canvas::new(550, 400);
        xt.loadmusic(2, 80, &mut cvl, &m);
        assert_eq!(xt.fase, 176, "the else branch is the live path");
        assert!(xt.loadedt[1], "stage 2 armed");
        assert_eq!(xt.pcontin, 0);
        assert!(!xt.mutem);
        assert!(!xt.mutes);
        // the draw ran first: same tint corners as framer
        assert_eq!(at(&cvl, 0, 0), [201, 184, 161], "loadmusic drew framer");

        // hold 0 plays the track and lands fase 6
        xt.loadmusic(2, 0, &mut Canvas::new(550, 400), &m);
        assert_eq!(xt.fase, 6);
        assert!(xt.loadedt[1]);
        assert_eq!(xt.pcontin, 0);
        assert!(!xt.mutem);
        assert!(!xt.mutes);
    }

    /// Armed stage-2 voice: a fresh mix through `loadmusic(2, 80)`
    /// posits the pinned mixer length (modslayer's pin), hold 0 starts
    /// playback, and a second load keeps the armed voice (the guard).
    #[test]
    fn loadmusic_builds_the_stage_track() {
        let mut xt = XtGraphics::new();
        let m = Medium::new();
        xt.loadmusic(2, 80, &mut Canvas::new(550, 400), &m);
        assert!(xt.loadedt[1], "stage 2 armed");
        let r = xt.stracks[1].as_ref().expect("stage 2 voice built");
        assert!(r.stream.is_some());
        assert_eq!(r.posit(), 2232930, "the pinned mixer length");
        assert_eq!(xt.fase, 176);
        assert!(!xt.stracks[1].as_ref().unwrap().playing);

        xt.loadmusic(2, 0, &mut Canvas::new(550, 400), &m);
        assert_eq!(xt.fase, 6);
        assert!(xt.stracks[1].as_ref().unwrap().playing);

        // the armed guard skips the rebuild (same voice object state)
        xt.stracks[1].as_mut().unwrap().stop();
        xt.loadmusic(2, 80, &mut Canvas::new(550, 400), &m);
        assert!(xt.loadedt[1]);
        assert!(!xt.stracks[1].as_ref().unwrap().playing);
    }

    /// The stageselect→loadingstage handoff: the head plays the
    /// `stages` jingle, the enter route stops it (and `loadingstage`
    /// stops both jingles).
    #[test]
    fn stageselect_handoff_stops_the_jingle() {
        let mut xt = loaded_xt();
        assert!(xt.stages.is_some());
        let mut m = Medium::new();
        let mut ctl = Control::new();
        let mut cp = CheckPoints::new();
        cp.stage = 1;

        // head plays the jingle
        xt.stageselect(&mut Canvas::new(550, 400), &mut cp, &mut m, &mut ctl);
        assert!(xt.stages.as_ref().unwrap().playing);

        // enter starts the load and stops the jingle
        ctl.handb = true;
        xt.stageselect(&mut Canvas::new(550, 400), &mut cp, &mut m, &mut ctl);
        assert_eq!(xt.fase, 5);
        assert!(!xt.stages.as_ref().unwrap().playing);

        // loadingstage stops both jingles (re-arm cars first)
        xt.cars.as_mut().unwrap().play();
        xt.stages.as_mut().unwrap().play();
        xt.loadingstage(1, &mut Canvas::new(550, 400), &m);
        assert!(!xt.cars.as_ref().unwrap().playing);
        assert!(!xt.stages.as_ref().unwrap().playing);
    }

    /// `carselect` opens with the `cars` jingle (xt:3358).
    #[test]
    fn carselect_opens_with_the_cars_jingle() {
        let mut xt = loaded_xt();
        assert!(xt.cars.is_some());
        let mut ctl = Control::new();
        let mut m = Medium::new();
        let mut t = Trackers::new();
        let mad = Madness::new(0);
        let mut car = Conto::place(&base_for(0), &m, &mut t, 0, 0, 0, 0);
        let mut cv = Canvas::new(550, 400);
        xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        assert!(xt.cars.as_ref().unwrap().playing);
    }

    /// `finish`'s enter route (xt:2265) stops the armed track.
    #[test]
    fn finish_enter_stops_the_track() {
        let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) = finish_rig(true, 2, 1);
        xt.loadedt[0] = true;
        let raw: &[u8] = include_bytes!("../../../../assets/music/stage1.zipo");
        let z = Zipo::read(raw).unwrap();
        let mut r = RadicalMod::new(&z.entries[0].data, 350, 8400, 135);
        r.play();
        xt.stracks[0] = Some(r);
        ctl.enter = true;
        xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
        assert_eq!(xt.fase, 10);
        assert!(!xt.stracks[0].as_ref().unwrap().playing);
    }

    /// `stageselect`'s key routing: enter/handb start the music load
    /// (fase 5, `m.trk` cleared); right steps to the next stage (fase 2)
    /// unless it is the newest unlocked stage — then the lock screen
    /// arms (fase 4, lockcnt 70); left steps back; the list ends hold.
    #[test]
    fn stageselect_routes_the_stage_list() {
        let mut xt = loaded_xt();
        let mut m = Medium::new();
        let mut ctl = Control::new();
        let mut cp = CheckPoints::new();
        cp.stage = 1;
        xt.unlocked = 2;

        // right below the unlocked edge: step and re-load
        ctl.right = true;
        xt.stageselect(&mut Canvas::new(550, 400), &mut cp, &mut m, &mut ctl);
        assert_eq!(cp.stage, 2);
        assert_eq!(xt.fase, 2, "the stage reload");
        assert!(!ctl.right);

        // right at the newest unlocked stage: the lock screen
        ctl.right = true;
        xt.stageselect(&mut Canvas::new(550, 400), &mut cp, &mut m, &mut ctl);
        assert_eq!(cp.stage, 2, "the stage held");
        assert_eq!(xt.fase, 4);
        assert_eq!(xt.lockcnt, 70, "the lock screen armed");

        // left steps back and re-loads
        xt.fase = 1;
        ctl.left = true;
        xt.stageselect(&mut Canvas::new(550, 400), &mut cp, &mut m, &mut ctl);
        assert_eq!(cp.stage, 1);
        assert_eq!(xt.fase, 2);
        assert!(!ctl.left);

        // left at the first stage is inert
        xt.fase = 1;
        ctl.left = true;
        xt.stageselect(&mut Canvas::new(550, 400), &mut cp, &mut m, &mut ctl);
        assert_eq!(cp.stage, 1);
        assert_eq!(xt.fase, 1, "no wrap at the start");
        assert!(ctl.left, "the key unconsumed");

        // the final stage ignores right (the caption is the masked band)
        ctl.left = false;
        cp.stage = 11;
        m.trk = true;
        ctl.right = true;
        xt.stageselect(&mut Canvas::new(550, 400), &mut cp, &mut m, &mut ctl);
        assert_eq!(cp.stage, 11, "no wrap at the end");
        assert!(ctl.right);

        // enter starts the music load
        ctl.handb = true;
        xt.stageselect(&mut Canvas::new(550, 400), &mut cp, &mut m, &mut ctl);
        assert_eq!(xt.fase, 5);
        assert!(!m.trk, "the track flag cleared");
        assert!(!ctl.handb);
    }

    // ────────────── the pause-machine slice ──────────────

    /// `carselect`'s 2D chrome vs JDK 1.8.0_40 (scratch `CarSelGt`,
    /// sc[0] = 4 at the stat panel): carsbg, selectcar, back/next, the
    /// statb plates, the six formula bars and the contin2 slot. The
    /// contour is seeded far off-camera so `ContO.d` culls it (the
    /// capture has no car); the label columns and the two drawcs rows
    /// are masked on both sides.
    /// The text row bands were re-cut from the ATLAS raster (the 13pt cap
    /// reaches one pixel above the band the older 8x8 stand-in needed): the
    /// rows outside the new bands are untouched, so the pin still carries the
    /// capture's chrome (C1 ledger: the shifted masked md5s).
    #[test]
    fn carselect_chrome_matches_the_jdk_with_the_car_culled() {
        let (mut xt, mut car, mut m, t, mad) = car_rig();
        car.z = -100_000;
        let mut cv = Canvas::new(550, 400);
        let mut ctl = Control::new();
        xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        assert_eq!(
            hex(&md5(&dump_masked_rects(
                &cv,
                &[(120, 136), (388, 400)],
                &[
                    (20, 97, 303, 350),
                    (283, 371, 303, 350),
                    (283, 400, 306, 307),
                    (283, 400, 321, 322),
                    (283, 400, 336, 337),
                ]
            ))),
            "7dfe2d1fa051c26e85000517191f560f",
            "carselect 2D chrome, car culled"
        );
    }

    /// `carselect`'s camera rig, idle pose, slide machine and enter
    /// routing: the contour parks at (0, -34-grat, 950) with the xz
    /// spin and the wzy wobble wrapping at -45; right/left arm the
    /// 20-frame slide (lift 100/frame, zy ±20, the car swap at flipo
    /// 10 dropping the next one in from -1100); enter starts the stage
    /// load (fase 2, camera rig off) only for playable cars and always
    /// consumes the keys.
    #[test]
    fn carselect_slides_and_routes() {
        let (mut xt, mut car, mut m, t, mad) = car_rig();
        let mut cv = Canvas::new(550, 400);
        let mut ctl = Control::new();

        xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        assert!(m.crs, "the car-select camera");
        assert_eq!(
            (m.x, m.y, m.z, m.xz, m.zy, m.ground),
            (-275, -500, -50, 0, 10, 470),
            "the camera rig"
        );
        assert_eq!(car.z, 950);
        assert_eq!(car.y, -34 - car.grat);
        assert_eq!(car.x, 0);
        assert_eq!(car.xz, 5, "the idle spin");
        assert_eq!(car.zy, 0);
        assert_eq!(car.wzy, -10, "the wobble");

        // four more idle calls wrap the wobble at -45 (=-50 → +45 → -5)
        for _ in 0..4 {
            xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        }
        assert_eq!(car.wzy, -5, "the wobble wrapped");
        assert_eq!(car.xz, 25, "the spin kept winding");

        // right arms the slide
        ctl.right = true;
        xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        assert_eq!(xt.flipo, 20);
        assert!(xt.nextc);
        assert!(!ctl.right);

        // the slide run arms the lock panel and lifts the car
        let y0 = car.y;
        xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        assert_eq!(xt.gatey, 300, "the lock panel re-armed");
        assert_eq!(xt.flipo, 19);
        assert_eq!(car.y, y0 - 100);
        assert_eq!(car.zy, 20);

        // down to the swap frame
        while xt.flipo > 10 {
            xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        }
        xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        assert_eq!(xt.flipo, 9, "the swap frame also decrements");
        assert_eq!(xt.sc[0], 5, "the slide advanced the pick");
        assert_eq!(car.z, 950);
        assert_eq!(car.zy, 0, "the rolled-in frame reset");

        // the idle chase back to 0
        while xt.flipo > 0 {
            xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        }
        assert_eq!(xt.flipo, 0);

        // enter on a playable car starts the stage load
        xt.sc[0] = 4;
        ctl.enter = true;
        xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        assert_eq!(xt.fase, 2);
        assert!(!m.crs, "the rig released");
        assert!(!ctl.enter);

        // a locked car (stage requirement 2 with unlocked 1) swallows it
        xt.fase = 7;
        xt.sc[0] = 5;
        ctl.handb = true;
        xt.carselect(&mut ctl, &mut car, &mad, &mut cv, &mut m, &t, None);
        assert_eq!(xt.fase, 7, "no start while locked");
        assert!(!ctl.handb);
    }

    // ────────────── the finish slice ──────────────

    /// `finish`'s four non-podium branches against JDK 1.8.0_40 (scratch
    /// `FinishGt`, the pattern backdrop assigned directly on both sides,
    /// fresh aflk): the loser/newest frame (`gameov` + the stage-1 hint),
    /// the winner-on-an-older-stage frame (`congrd` at 117), the
    /// car-less newest unlock (`congrd` at 30 + the pin-60 lines) and
    /// the stage-11 endgame (the `congrd` jitter band masked, the
    /// radicalplay page pinned). drawcs rows masked per branch.
    /// The text row bands were re-cut from the ATLAS raster (the 13pt cap
    /// reaches one pixel above the band the older 8x8 stand-in needed): the
    /// rows outside the new bands are untouched, so the pin still carries the
    /// capture's chrome (C1 ledger: the shifted masked md5s).
    #[test]
    fn finish_branches_match_the_jdk() {
        // loser on the newest stage (stage 1 → the hint line)
        let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) = finish_rig(false, 1, 1);
        xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
        assert_eq!(xt.pin, 100);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[(159, 172), (197, 210)]))),
            "12c9e9ae37f7d6776d0370180d4f1c78",
            "loser frame"
        );

        // winner on an older stage
        let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) = finish_rig(true, 2, 1);
        xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
        assert_eq!(xt.pin, 100);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[(159, 172)]))),
            "3d0dfc0eab43fe695b20d34db3c87d7a",
            "winner, older stage"
        );

        // winner on the newest stage, car-less (stage 1)
        let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) = finish_rig(true, 1, 1);
        xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
        assert_eq!(xt.pin, 60);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[(72, 84), (170, 184), (267, 280)]))),
            "9f02bee015584b628a4982d0f5bbd881",
            "newest unlock, no car"
        );

        // the stage-11 endgame
        let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) = finish_rig(true, 11, 11);
        xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
        assert_eq!(xt.pin, 40);
        assert_eq!(
            hex(&md5(&dump_masked(
                &cv,
                &[(25, 122), (139, 156), (190, 204), (262, 274)]
            ))),
            "e583ad1d87e7ccec52b7f89d3256eaee",
            "stage-11 endgame"
        );
    }

    /// `finish`'s podium machine: the per-stage picks (stage 2 → car 5
    /// at y 60, 4 → 6/40, 6 → 7/25, 8 → 8/70, 10 → 9/80) with
    /// `pin = 0`, the `sc[0]` write, the carselect camera rig at ground
    /// 2470 and the car pose; the frame color flips with aflk.
    #[test]
    fn finish_podium_picks_and_frame_flip() {
        let picks = [(2, 5, 60), (4, 6, 40), (6, 7, 25), (8, 8, 70), (10, 9, 80)];
        for (stage, car_slot, y) in picks {
            let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) =
                finish_rig(true, stage, stage);
            xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
            assert_eq!(xt.pin, 0, "stage {stage} pin");
            assert_eq!(xt.sc[0], car_slot, "stage {stage} car pick");
            assert_eq!(cars[car_slot as usize].y, y, "stage {stage} podium y");
            assert_eq!(cars[car_slot as usize].z, 1500);
            assert_eq!(cars[car_slot as usize].x, 0);
            assert!(m.crs);
            assert_eq!(m.ground, 2470);
            assert_eq!(cars[car_slot as usize].zy, 0);
            assert_eq!(cars[car_slot as usize].wzy, -10);
            assert_eq!(cars[car_slot as usize].xz, 5, "stage {stage} idle spin");
        }

        // the frame color flip: aflk false → (255,0,0), true → (0,128,255)
        let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) = finish_rig(true, 2, 2);
        xt.aflk = false;
        xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
        let i = (167 * 550 + 274) * 3;
        assert_eq!([cv.px[i], cv.px[i + 1], cv.px[i + 2]], [255, 0, 0]);
        // aflk flipped to true at the end of that call
        let (mut xt2, mut cp2, mut cars2, mut cv2, mut m2, t2, mut ctl2) = finish_rig(true, 2, 2);
        xt2.aflk = true;
        xt2.finish(&mut cp2, &mut cars2, &mut cv2, &mut m2, &t2, &mut ctl2);
        let i = (167 * 550 + 274) * 3;
        assert_eq!([cv2.px[i], cv2.px[i + 1], cv2.px[i + 2]], [0, 128, 255]);
    }

    /// `finish`'s enter route: always back to the menu (fase 10); the
    /// newest-stage win advances `cp.stage`/`unlocked` (never at stage
    /// 11); older wins leave them.
    #[test]
    fn finish_enter_advances_the_unlock() {
        let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) = finish_rig(true, 1, 1);
        ctl.enter = true;
        xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
        assert_eq!(xt.fase, 10);
        assert_eq!(cp.stage, 2, "the unlock stepped the stage");
        assert_eq!(xt.unlocked, 2);
        assert!(!ctl.enter);

        // not the newest: no advance
        let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) = finish_rig(true, 2, 1);
        ctl.handb = true;
        xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
        assert_eq!(xt.fase, 10);
        assert_eq!(cp.stage, 1, "older win keeps the stage");
        assert_eq!(xt.unlocked, 2);
        assert!(!ctl.handb);

        // the finale caps: unlocked stays 11
        let (mut xt, mut cp, mut cars, mut cv, mut m, t, mut ctl) = finish_rig(true, 11, 11);
        ctl.enter = true;
        xt.finish(&mut cp, &mut cars, &mut cv, &mut m, &t, &mut ctl);
        assert_eq!(cp.stage, 11);
        assert_eq!(xt.unlocked, 11);
    }
}
