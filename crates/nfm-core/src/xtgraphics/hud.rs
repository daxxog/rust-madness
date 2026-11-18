//! The race HUD (`stat`, `drawstat`, `arrow`, `ctachm`) and the
//! pause/flex machines (`pauseimage`, `fleximage`, `pausedgame`). The
//! JDK color-space helpers the flex fade needs (`rgb_to_hsb`/
//! `hsb_to_rgb`/`flex_darken`) live here with their only caller.

use super::*;

/// `java.awt.Color.RGBtoHSB(r, g, b, null)` (JDK 1.8): the exact f32
/// math, hue normalised to [0,1). Used by `fleximage`'s re-hue pass
/// (`getHSBColor(0.7F, s, v)`).
pub(crate) fn rgb_to_hsb(r: i32, g: i32, b: i32) -> (f32, f32, f32) {
    let cmax = if r > g { r } else { g }.max(b);
    let cmin = if r < g { r } else { g }.min(b);
    let brightness = cmax as f32 / 255.0f32;
    let saturation = if cmax != 0 {
        (cmax - cmin) as f32 / cmax as f32
    } else {
        0.0f32
    };
    let hue = if saturation == 0.0f32 {
        0.0f32
    } else {
        let redc = (cmax - r) as f32 / (cmax - cmin) as f32;
        let greenc = (cmax - g) as f32 / (cmax - cmin) as f32;
        let bluec = (cmax - b) as f32 / (cmax - cmin) as f32;
        let mut h = if r == cmax {
            bluec - greenc
        } else if g == cmax {
            2.0f32 + redc - bluec
        } else {
            4.0f32 + greenc - redc
        };
        h /= 6.0f32;
        if h < 0.0f32 {
            h += 1.0f32;
        }
        h
    };
    (hue, saturation, brightness)
}

/// `java.awt.Color.HSBtoRGB(h, s, b)` (JDK 1.8): the exact f32 math and
/// the +0.5f rounding casts.
pub(crate) fn hsb_to_rgb(hue: f32, saturation: f32, brightness: f32) -> (i32, i32, i32) {
    if saturation == 0.0f32 {
        let v = (brightness * 255.0f32 + 0.5f32) as i32;
        return (v, v, v);
    }
    let h = (hue - hue.floor()) * 6.0f32;
    let f = h - h.floor();
    let p = brightness * (1.0f32 - saturation);
    let q = brightness * (1.0f32 - saturation * f);
    let t = brightness * (1.0f32 - (saturation * (1.0f32 - f)));
    let v = (brightness * 255.0f32 + 0.5f32) as i32;
    let pp = (p * 255.0f32 + 0.5f32) as i32;
    let qq = (q * 255.0f32 + 0.5f32) as i32;
    let tt = (t * 255.0f32 + 0.5f32) as i32;
    match h as i32 {
        0 => (v, tt, pp),
        1 => (qq, v, pp),
        2 => (pp, v, tt),
        3 => (pp, qq, v),
        4 => (tt, pp, v),
        _ => (v, pp, qq),
    }
}

/// `fleximage`'s per-pixel darkening arithmetic (the `m.random() > 0.2`
/// branch): `(int)(r - r*0.4)`, `(int)(g - g*0.35)` and
/// `(int)(b - b*(0.25 + (float)step/30.0F))` with the 225/0 clamps —
/// the f64 expression widened from the f32 step factor, exactly like
/// the source.
pub(crate) fn flex_darken(r: i32, g: i32, b: i32, step: i32) -> (i32, i32, i32) {
    let blue = 0.25f64 + (step as f32 / 30.0f32) as f64;
    let nr = (r as f64 - r as f64 * 0.4) as i32;
    let ng = (g as f64 - g as f64 * 0.35) as i32;
    let nb = (b as f64 - b as f64 * blue) as i32;
    (nr.clamp(0, 225), ng.clamp(0, 225), nb.clamp(0, 225))
}

impl XtGraphics {
    /// `xtGraphics.stat(Madness, CheckPoints, Control, boolean, Graphics)`
    /// (xt:1515-2078): one frame of the in-race HUD, plus every result
    /// screen that rides the same call. `follow` is the source's `var4`
    /// (view == 0): only the follow view announces the arrow-target
    /// switch. The continue screens hand out with `self.fase = -2`, the
    /// result with `-6` (the source's `this.fase` writes at xt:1607 and
    /// xt:1620). AudioClip calls are omitted (the audio slice).
    pub fn stat(
        &mut self,
        mad: &mut Madness,
        cp: &mut CheckPoints,
        ctl: &mut Control,
        follow: bool,
        cv: &mut Canvas,
        m: &mut Medium,
    ) {
        let mut done = false;

        // xt:1518 — every car wasted: the "You Won" panel.
        if self.wasted == 4 {
            if m.flex != 2 {
                let (w, h) = (self.youwastedem.w as i32, self.youwastedem.h as i32);
                cv.fill_rect(166, 70, w, h, clampc(m.csky));
                cv.draw_rect(166, 70, w, h, clampc(m.cfade));
            }
            cv.draw_image(&self.youwastedem, 166, 70);
            if self.aflk {
                self.drawcs(
                    cv,
                    11,
                    120,
                    "You Won, all cars have been wasted!",
                    [0, 0, 0],
                    0,
                    m,
                );
                self.aflk = false;
            } else {
                self.drawcs(
                    cv,
                    11,
                    120,
                    "You Won, all cars have been wasted!",
                    [0, 128, 255],
                    0,
                    m,
                );
                self.aflk = true;
            }
            self.drawcs(
                cv,
                11,
                350,
                "Press  [ Enter ]  to continue",
                [0, 0, 0],
                0,
                m,
            );
            cp.haltall = true;
            done = true;
            self.winner = true; // xt:1538
        }

        // xt:1540 — the player wrecked (the burning counter reached 8).
        if !done && mad.dest && self.cntwis == 8 {
            if m.flex != 2 {
                let (w, h) = (self.yourwasted.w as i32, self.yourwasted.h as i32);
                cv.fill_rect(172, 70, w, h, clampc(m.csky));
                cv.draw_rect(172, 70, w, h, clampc(m.cfade));
            }
            cv.draw_image(&self.yourwasted, 172, 70);
            self.drawcs(
                cv,
                11,
                350,
                "Press  [ Enter ]  to continue",
                [0, 0, 0],
                0,
                m,
            );
            done = true;
            self.winner = false;
        }

        // xt:1553 — the finish boxes (the player's or a competitor's win).
        if !done {
            for i in 0..5usize {
                if cp.clear[i] == cp.nlaps * cp.nsp && cp.pos[i] == 0 {
                    if i == 0 {
                        if m.flex != 2 {
                            let (w, h) = (self.youwon.w as i32, self.youwon.h as i32);
                            cv.fill_rect(208, 70, w, h, clampc(m.csky));
                            cv.draw_rect(208, 70, w, h, clampc(m.cfade));
                        }
                        cv.draw_image(&self.youwon, 208, 70);
                        if self.aflk {
                            self.drawcs(
                                cv,
                                11,
                                120,
                                "You finished first, nice job!",
                                [0, 0, 0],
                                0,
                                m,
                            );
                            self.aflk = false;
                        } else {
                            self.drawcs(
                                cv,
                                11,
                                120,
                                "You finished first, nice job!",
                                [0, 128, 255],
                                0,
                                m,
                            );
                            self.aflk = true;
                        }
                        self.winner = true;
                    } else {
                        if m.flex != 2 {
                            let (w, h) = (self.youlost.w as i32, self.youlost.h as i32);
                            cv.fill_rect(211, 70, w, h, clampc(m.csky));
                            cv.draw_rect(211, 70, w, h, clampc(m.cfade));
                        }
                        cv.draw_image(&self.youlost, 211, 70);
                        let msg = format!(
                            "{} finished first, race over!",
                            self.names[self.sc[i] as usize]
                        );
                        if self.aflk {
                            self.drawcs(cv, 11, 120, &msg, [0, 0, 0], 0, m);
                            self.aflk = false;
                        } else {
                            self.drawcs(cv, 11, 120, &msg, [0, 128, 255], 0, m);
                            self.aflk = true;
                        }
                        self.winner = false;
                    }
                    self.drawcs(
                        cv,
                        11,
                        350,
                        "Press  [ Enter ]  to continue",
                        [0, 0, 0],
                        0,
                        m,
                    );
                    cp.haltall = true;
                    done = true;
                }
            }
        }

        // xt:1598 — the screen's continue: hold on the win/wreck panels,
        // straight to the replay on the race-over enter.
        if done {
            self.holdcnt += 1;
            if ctl.enter || self.holdcnt > 250 {
                self.fase = -2;
                ctl.enter = false;
            }
        } else {
            if self.holdcnt != 0 {
                self.holdcnt = 0;
            }
            if ctl.enter {
                if self.loadedt[(cp.stage - 1) as usize]
                    && let Some(r) = self.stracks[(cp.stage - 1) as usize].as_mut()
                {
                    r.stop();
                }
                self.fase = -6;
                ctl.enter = false;
            }
        }

        // xt:1625 — the arrow-target switch announces on the follow view.
        if follow && cp.stage != 10 && self.arrace != ctl.arrace {
            self.arrace = ctl.arrace;
            if self.arrace {
                self.wasay = true;
                self.say = " Arrow now pointing at  Cars  <".to_string();
                self.tcnt = -5;
            }
            if !self.arrace {
                self.wasay = false;
                self.say = " Arrow now pointing at  Track  <".to_string();
                self.tcnt = -5;
                self.cntan = 20;
            }
        }

        // xt:1627-1712 — the follow-view HUD. The countdown gate belongs
        // to the ARROW/missed block only; the panel stack (and drawstat)
        // runs on every follow pass, INCLUDING the starcnt-35 landing and
        // under a result panel (the source's `var4` = follow wraps the
        // whole HUD; `var6` = done gates only the countdown/messages).
        if follow {
            if !done && cp.stage != 10 && self.starcnt == 0 {
                let arr = self.arrace;
                self.arrow(cv, mad.point, mad.missedcp, cp, arr, m);

                // xt:1646 — the checkpoint-missed / wrong-way counters.
                if !self.arrace && self.auscnt == 45 && mad.capcnt == 0 {
                    if mad.missedcp > 0 {
                        if mad.missedcp > 15 && mad.missedcp < 50 {
                            if self.flk {
                                self.drawcs(cv, 11, 70, "CheckPoint Missed!", [255, 150, 0], 2, m);
                            } else {
                                self.drawcs(cv, 11, 70, "CheckPoint Missed!", [255, 0, 0], 2, m);
                            }
                        }
                        mad.missedcp += 1;
                        if mad.missedcp == 70 {
                            mad.missedcp = -2;
                        }
                    } else if mad.mtouch && self.cntovn < 70 {
                        if self.ana.abs() > 100 {
                            self.cntan += 1;
                        } else if self.cntan != 0 {
                            self.cntan -= 1;
                        }
                        if self.cntan > 40 {
                            self.cntovn += 1;
                            self.cntan = 40;
                            if self.flk {
                                self.drawcs(cv, 11, 70, "Wrong Way!", [255, 150, 0], 2, m);
                                self.flk = false;
                            } else {
                                self.drawcs(cv, 11, 70, "Wrong Way!", [255, 0, 0], 2, m);
                                self.flk = true;
                            }
                        }
                    }
                }
            }

            // xt:1690 — the panel stack: full draw on flex 0/1, delta
            // redraws once the flex machine settles at 2.
            if m.flex != 2 {
                cv.draw_image(&self.dmg, 360, 7);
                cv.draw_image(&self.pwr, 360, 27);
                cv.draw_image(&self.lap, 19, 7);
                cv.draw_string(
                    &format!("{} / {}", mad.nlaps + 1, cp.nlaps),
                    51,
                    18,
                    [0, 0, 100],
                    11,
                );
                cv.draw_image(&self.was, 92, 7);
                cv.draw_string(&format!("{} / 4", cp.wasted), 150, 18, [0, 0, 100], 11);
                cv.draw_image(&self.pos, 42, 27);
                cv.draw_image(&self.rank[cp.pos[mad.im] as usize], 110, 28);
                m.flex += 1;
            } else {
                if self.posit != cp.pos[mad.im] {
                    cv.draw_image(&self.rank[cp.pos[mad.im] as usize], 110, 28);
                    self.posit = cp.pos[mad.im];
                }
                if self.wasted != cp.wasted {
                    cv.fill_rect(150, 8, 30, 10, clampc(m.csky));
                    cv.draw_string(&format!("{} / 4", cp.wasted), 150, 18, [0, 0, 100], 11);
                    self.wasted = cp.wasted;
                }
                if self.laps != mad.nlaps {
                    cv.fill_rect(51, 8, 40, 10, clampc(m.csky));
                    cv.draw_string(
                        &format!("{} / {}", mad.nlaps + 1, cp.nlaps),
                        51,
                        18,
                        [0, 0, 100],
                        11,
                    );
                    self.laps = mad.nlaps;
                }
            }
            self.drawstat(cv, MAXMAG[mad.cn], mad.hitmag, mad.newcar, mad.power, m);
        }

        if !done {
            // xt:1744 — the countdown number, its swaps and its voice:
            // three at 35, two/one/go with the gocnt swaps (xt:1718-1742).
            if self.starcnt != 0 {
                if self.starcnt == 35
                    && !self.mutes
                    && let Some(t) = self.three.as_mut()
                {
                    t.play();
                }
                if self.starcnt == 24 {
                    self.gocnt = 2;
                    if !self.mutes
                        && let Some(t) = self.two.as_mut()
                    {
                        t.play();
                    }
                }
                if self.starcnt == 13 {
                    self.gocnt = 1;
                    if !self.mutes
                        && let Some(t) = self.one.as_mut()
                    {
                        t.play();
                    }
                }
                if self.starcnt == 2 {
                    self.gocnt = 0;
                    if !self.mutes
                        && let Some(t) = self.go.as_mut()
                    {
                        t.play();
                    }
                }
                if self.gocnt != 0 {
                    cv.draw_image(&self.cntdn[self.gocnt as usize], 260, 50);
                } else {
                    cv.draw_image(&self.cntdn[0], 238, 50);
                }
            }

            // xt:1752 — the stunt-message alternator arm.
            if self.looped != 0 && mad.loop_ == 2 {
                self.looped = 0;
            }

            // xt:1756 — the power-low nag.
            if mad.power < 45.0 {
                if self.tcnt == 30 && self.auscnt == 45 && mad.mtouch && mad.capcnt == 0 {
                    if self.pwcnt < 70 {
                        if self.looped != 3 {
                            if self.pwflk {
                                self.drawcs(
                                    cv,
                                    11,
                                    110,
                                    "Power low, perform stunt!",
                                    [0, 0, 200],
                                    0,
                                    m,
                                );
                                self.pwflk = false;
                            } else {
                                self.drawcs(
                                    cv,
                                    11,
                                    110,
                                    "Power low, perform stunt!",
                                    [255, 100, 0],
                                    0,
                                    m,
                                );
                                self.pwflk = true;
                            }
                        } else if self.pwflk {
                            self.drawcs(
                                cv,
                                11,
                                110,
                                ">> Press Enter for game instructions! <<",
                                [0, 0, 200],
                                0,
                                m,
                            );
                            self.pwflk = false;
                        } else {
                            self.drawcs(
                                cv,
                                11,
                                110,
                                ">> Press Enter for game instructions! <<",
                                [255, 100, 0],
                                0,
                                m,
                            );
                            self.pwflk = true;
                        }
                    }
                    self.pwcnt += 1;
                    if self.pwcnt == 300 {
                        self.pwcnt = 0;
                        if self.looped != 0 {
                            self.looped += 1;
                            if self.looped == 4 {
                                self.looped = 2;
                            }
                        }
                    }
                }
            } else if self.pwcnt != 0 {
                self.pwcnt = 0;
            }

            // xt:1789 — the message lines: `say` while grounded, `asay`
            // while a stunt message runs, "Bad Landing!" in the air.
            if mad.capcnt == 0 {
                if self.tcnt < 30 {
                    // the wasay split draws identical colors in the
                    // decompile (a flattened ternary) — one call here.
                    if self.tflk {
                        self.drawcs(cv, 11, 105, &self.say, [0, 0, 0], 0, m);
                        self.tflk = false;
                    } else if !self.wasay {
                        self.drawcs(cv, 11, 105, &self.say, [0, 128, 255], 0, m);
                        self.tflk = true;
                    } else {
                        self.drawcs(cv, 11, 105, &self.say, [255, 128, 0], 0, m);
                        self.tflk = true;
                    }
                    self.tcnt += 1;
                } else if self.wasay {
                    self.wasay = false;
                }
                if self.auscnt < 45 {
                    if self.aflk {
                        self.drawcs(cv, 11, 85, &self.asay, [98, 176, 255], 0, m);
                        self.aflk = false;
                    } else {
                        self.drawcs(cv, 11, 85, &self.asay, [0, 128, 255], 0, m);
                        self.aflk = true;
                    }
                    self.auscnt += 1;
                }
            } else if self.tflk {
                self.drawcs(cv, 11, 110, "Bad Landing!", [0, 0, 200], 0, m);
                self.tflk = false;
            } else {
                self.drawcs(cv, 11, 110, "Bad Landing!", [255, 100, 0], 0, m);
                self.tflk = true;
            }

            // xt:1848 — the stunt line, built once at trcnt == 10.
            if mad.trcnt == 10 {
                self.loop_ = String::new();
                self.spin = String::new();
                self.asay = String::new();

                let mut turns = 0i32;
                while mad.travzy > 225 {
                    mad.travzy -= 360;
                    turns += 1;
                }
                while mad.travzy < -225 {
                    mad.travzy += 360;
                    turns -= 1;
                }
                self.loop_ = match turns {
                    1 => "Forward loop".to_string(),
                    2 => "double Forward".to_string(),
                    3 => "triple Forward".to_string(),
                    -1 => "Backloop".to_string(),
                    -2 => "double Back".to_string(),
                    -3 => "triple Back".to_string(),
                    t if t >= 4 => "massive Forward looping".to_string(),
                    t if t <= -4 => "massive Back looping".to_string(),
                    _ => {
                        if mad.ftab && mad.btab {
                            "Tabletop and reversed Tabletop".to_string()
                        } else if mad.ftab || mad.btab {
                            "Tabletop".to_string()
                        } else {
                            String::new()
                        }
                    }
                };
                if turns > 0 && mad.btab {
                    self.loop_ = format!("Hanged {}", self.loop_);
                }
                if turns < 0 && mad.ftab {
                    self.loop_ = format!("Hanged {}", self.loop_);
                }
                if !self.loop_.is_empty() {
                    self.asay = format!("{} {}", self.asay, self.loop_);
                }

                let mut var10 = 0i32;
                mad.travxy = mad.travxy.abs();
                while mad.travxy > 270 {
                    mad.travxy -= 360;
                    var10 += 1;
                }
                self.spin = if var10 == 0 && mad.rtab {
                    if self.loop_.is_empty() {
                        "Tabletop".to_string()
                    } else {
                        "Flipside".to_string()
                    }
                } else {
                    match var10 {
                        1 => "Rollspin".to_string(),
                        2 => "double Rollspin".to_string(),
                        3 => "triple Rollspin".to_string(),
                        t if t >= 4 => "massive Roll spinning".to_string(),
                        _ => String::new(),
                    }
                };

                let mut var10 = 0i32;
                let mut beyond = false;
                mad.travxz = mad.travxz.abs();
                while mad.travxz > 90 {
                    mad.travxz -= 180;
                    var10 += 180;
                    if var10 > 900 {
                        var10 = 900;
                        beyond = true;
                    }
                }
                if var10 != 0 {
                    if self.loop_.is_empty() && self.spin.is_empty() {
                        self.asay = format!("{} {}", self.asay, var10);
                        if beyond {
                            self.asay = format!("{} and beyond", self.asay);
                        }
                    } else {
                        if !self.spin.is_empty() {
                            if self.loop_.is_empty() {
                                self.asay = format!("{} {}", self.asay, self.spin);
                            } else {
                                self.asay = format!("{} with {}", self.asay, self.spin);
                            }
                        }
                        self.asay = format!("{} by {}", self.asay, var10);
                        if beyond {
                            self.asay = format!("{} and beyond", self.asay);
                        }
                    }
                } else if !self.spin.is_empty() {
                    if self.loop_.is_empty() {
                        self.asay = format!("{} {}", self.asay, self.spin);
                    } else {
                        self.asay = format!("{} by {}", self.asay, self.spin);
                    }
                }

                if !self.asay.is_empty() {
                    self.auscnt -= 15;
                }
                if !self.loop_.is_empty() {
                    self.auscnt -= 25;
                }
                if !self.spin.is_empty() {
                    self.auscnt -= 25;
                }
                if var10 != 0 {
                    self.auscnt -= 25;
                }
                if self.auscnt < 45 {
                    // xt:1987 — the stunt powerup sting, mutes-gated.
                    if !self.mutes
                        && let Some(p) = self.powerup.as_mut()
                    {
                        p.play();
                    }
                    if self.auscnt < -20 {
                        self.auscnt = -20;
                    }
                    let mut var9 = 0usize;
                    if mad.powerup > 20.0 {
                        var9 = 1;
                    }
                    if mad.powerup > 40.0 {
                        var9 = 2;
                    }
                    if mad.powerup > 150.0 {
                        var9 = 3;
                    }
                    if mad.surfer {
                        let pick = (m.random() * 3.0) as usize;
                        self.asay = format!(" {}{}", self.adj[4][pick], self.asay);
                    }
                    let pick = (m.random() * 3.0) as usize;
                    if var9 != 3 {
                        self.asay =
                            format!("{}{}{}", self.adj[var9][pick], self.asay, self.exlm[var9]);
                    } else {
                        self.asay = self.adj[var9][pick].to_string();
                    }
                    if !self.wasay {
                        self.tcnt = self.auscnt;
                        if mad.power != 98.0 {
                            self.say =
                                format!("Power Up {}%", (100.0f32 * mad.powerup / 98.0) as i32);
                        } else {
                            self.say = "Power To The MAX".to_string();
                        }
                    }
                }
            }

            // xt:2033 — the fixed-car announce.
            if mad.newcar {
                if !self.wasay {
                    self.say = "Car Fixed".to_string();
                    self.tcnt = 0;
                }
                self.crashup = !self.crashup;
            }

            // xt:2043 — the checkpoint line.
            if self.clear != mad.clear && mad.clear != 0 {
                if !self.wasay {
                    self.say = "CheckPoint".to_string();
                    self.tcnt = 15;
                }
                self.clear = mad.clear;
                // xt:2049 — the checkpoint sting, mutes-gated.
                if !self.mutes
                    && let Some(c) = self.checkpoint.as_mut()
                {
                    c.play();
                }
                self.cntovn = 0;
                if self.cntan != 0 {
                    self.cntan = 0;
                }
            }

            // xt:2060 — the wasted-car lines.
            for var13 in 1..5usize {
                if self.dested[var13] != cp.dested[var13] {
                    self.dested[var13] = cp.dested[var13];
                    if self.dested[var13] == 1 {
                        self.wasay = true;
                        self.say =
                            format!("{} has been wasted!", self.names[self.sc[var13] as usize]);
                        self.tcnt = -15;
                    }
                    if self.dested[var13] == 2 {
                        self.wasay = true;
                        self.say = format!("You wasted {}!", self.names[self.sc[var13] as usize]);
                        self.tcnt = -15;
                    }
                }
            }
        }
    }

    /// `xtGraphics.drawstat(Graphics, int, int, boolean, float)`
    /// (xt:2730-2874): the damage and power bars, both pure fillPolygon
    /// work. The damage bar flashes over `dmcnt` once it crosses 70; the
    /// power bar's width is the raw power (98 = full).
    pub fn drawstat(
        &mut self,
        cv: &mut Canvas,
        maxmag: i32,
        hitmag: i32,
        newcar: bool,
        power: f32,
        m: &Medium,
    ) {
        let mut xs_;
        let mut ys_;

        if newcar {
            xs_ = [423, 423, 520, 520];
            ys_ = [11, 19, 19, 11];
            cv.fill_polygon(&xs_, &ys_, clampc(m.csky));
        }

        let hitmag = if hitmag > maxmag { maxmag } else { hitmag };
        let width = (98.0f32 * (hitmag as f32 / maxmag as f32)) as i32;
        xs_ = [422, 422, 422 + width, 422 + width];
        ys_ = [11, 20, 20, 11];
        let mut r = 244i32;
        let mut g = 244i32;
        let mut b = 11i32;
        if width > 33 {
            g = (244.0f32 - 233.0f32 * ((width - 33) as f32 / 65.0f32)) as i32;
        }
        if width > 70 {
            if self.dmcnt < 10 {
                if self.dmflk {
                    g = 170;
                    self.dmflk = false;
                } else {
                    self.dmflk = true;
                }
            }
            self.dmcnt += 1;
            if self.dmcnt as f64 > 167.0 - width as f64 * 1.5 {
                self.dmcnt = 0;
            }
        }
        r = lift(r, m.snap[0]);
        g = lift(g, m.snap[1]);
        b = lift(b, m.snap[2]);
        cv.fill_polygon(&xs_, &ys_, clampc([r, g, b]));

        xs_ = [
            422,
            422,
            (422.0f32 + power) as i32,
            (422.0f32 + power) as i32,
        ];
        ys_ = [31, 40, 40, 31];
        let mut pr = 128i32;
        if power == 98.0f32 {
            pr = 64;
        }
        // the source computes the green channel in double here (its own
        // inconsistency; kept)
        let mut pg = (190.0f64 + power as f64 * 0.37) as i32;
        let mut pb = 244i32;
        if self.auscnt < 45 && self.aflk {
            pr = 128;
            pg = 244;
            pb = 244;
        }
        pr = lift(pr, m.snap[0]);
        pg = lift(pg, m.snap[1]);
        pb = lift(pb, m.snap[2]);
        cv.fill_polygon(&xs_, &ys_, clampc([pr, pg, pb]));

        // xt:2865-2874 — the settled-flex tail: the empty power-bar region
        // from the current fill to the bar's right edge is repainted in the
        // stage sky colour (the delta-redraw frame clears the old bar).
        if m.flex == 2 && power != 98.0f32 {
            xs_ = [
                (422.0f32 + power) as i32,
                (422.0f32 + power) as i32,
                520,
                520,
            ];
            ys_ = [31, 39, 39, 31];
            cv.fill_polygon(&xs_, &ys_, clampc(m.csky));
        }
    }

    /// `xtGraphics.arrow(Graphics, int, int, CheckPoints, boolean)`
    /// (xt:432-713): the guidance arrow — a 7-point shape in camera space
    /// (depths 650..810, floating at y = -90) rotated to the eased bearing
    /// and projected. `arrace` switches the aim from the next checkpoint to
    /// the nearest alive car (and prints its name). AudioClip calls are
    /// omitted (the audio slice).
    #[allow(clippy::too_many_arguments)]
    pub fn arrow(
        &mut self,
        cv: &mut Canvas,
        point: i32,
        missedcp: i32,
        cp: &CheckPoints,
        arrace: bool,
        m: &Medium,
    ) {
        let mut px = [0i32; 7];
        let mut py_ = [-90i32; 7];
        let mut pd = [0i32; 7];
        let cx: i32 = 275;
        let cz: i32 = 700;

        px[0] = cx;
        pd[0] = cz + 110;
        px[1] = cx - 35;
        pd[1] = cz + 50;
        px[2] = cx - 15;
        pd[2] = cz + 50;
        px[3] = cx - 15;
        pd[3] = cz - 50;
        px[4] = cx + 15;
        pd[4] = cz - 50;
        px[5] = cx + 15;
        pd[5] = cz + 50;
        px[6] = cx + 35;
        pd[6] = cz + 50;

        let mut var17;
        if !arrace {
            // the bearing to the next checkpoint (xt:446-450)
            let side: i32 = if cp.x[point as usize] - cp.opx[0] >= 0 {
                180
            } else {
                0
            };
            var17 = ((90 + side) as f64
                + ((cp.z[point as usize] - cp.opz[0]) as f64
                    / (cp.x[point as usize] - cp.opx[0]) as f64)
                    .atan()
                    / (std::f64::consts::PI / 180.0)) as i32;
        } else {
            // the nearest alive car (xt:475-491), skipping the onscreen
            // ones once one has been seen
            let mut near = 0usize;
            let mut best = -1i32;
            let mut seen = false;
            let mut var16 = 1usize;
            loop {
                let d = py(
                    cp.opx[0] / 100,
                    cp.opx[var16] / 100,
                    cp.opz[0] / 100,
                    cp.opz[var16] / 100,
                );
                if (d < best || best == -1)
                    && (!seen || cp.onscreen[var16] != 0)
                    && cp.dested[var16] == 0
                {
                    near = var16;
                    best = d;
                    if cp.onscreen[var16] != 0 {
                        seen = true;
                    }
                }
                var16 += 1;
                if var16 >= 5 {
                    break;
                }
            }
            let side: i32 = if cp.opx[near] - cp.opx[0] >= 0 {
                180
            } else {
                0
            };
            var17 = ((90 + side) as f64
                + ((cp.opz[near] - cp.opz[0]) as f64 / (cp.opx[near] - cp.opx[0]) as f64).atan()
                    / (std::f64::consts::PI / 180.0)) as i32;
            self.drawcs(
                cv,
                11,
                13,
                "[                              ]",
                [76, 67, 240],
                0,
                m,
            );
            self.drawcs(
                cv,
                11,
                13,
                self.names[self.sc[near] as usize],
                [0, 0, 0],
                0,
                m,
            );
        }

        var17 += m.xz;
        while var17 < 0 {
            var17 += 360;
        }
        while var17 > 180 {
            var17 -= 360;
        }

        // the swing clamps (two-sided in the source): the track aim
        // reaches further than the car aim
        if !arrace {
            var17 = var17.clamp(-130, 130);
        } else {
            var17 = var17.clamp(-100, 100);
        }

        // the eased spin toward the bearing (xt:536-556)
        if (self.ana - var17).abs() < 180 {
            if (self.ana - var17).abs() < 10 {
                self.ana = var17;
            } else if self.ana < var17 {
                self.ana += 10;
            } else {
                self.ana -= 10;
            }
        } else {
            if var17 < 0 {
                self.ana += 15;
                if self.ana > 180 {
                    self.ana -= 360;
                }
            }
            if var17 > 0 {
                self.ana -= 15;
                if self.ana < -180 {
                    self.ana += 360;
                }
            }
        }

        rot(&mut px, &mut pd, cx, cz, self.ana, 7, m);
        let var17 = self.ana.abs();

        if !arrace {
            // the track arrow hides dead-ahead and when nothing else
            // draws it
            if var17 > 7 || missedcp > 0 || missedcp == -2 || self.cntan != 0 {
                for i in 0..7 {
                    px[i] = xs(px[i], pd[i], m);
                    py_[i] = ys(py_[i], pd[i], m);
                }
                let mut r = lift(190, m.snap[0]);
                let mut g = lift(255, m.snap[1]);
                let mut b = 0i32;
                if missedcp <= 0 {
                    // near-forward it dissolves into the sky; a far
                    // swing lifts the red toward the snap-white
                    if var17 <= 45 && missedcp != -2 && self.cntan == 0 {
                        r = (r * var17 + m.csky[0] * (45 - var17)) / 45;
                        g = (g * var17 + m.csky[1] * (45 - var17)) / 45;
                        b = (b * var17 + m.csky[2] * (45 - var17)) / 45;
                    }
                    if var17 >= 90 {
                        let white = lift(255, m.snap[0]);
                        r = (r * (140 - var17) + white * (var17 - 90)) / 50;
                        if r > 255 {
                            r = 255;
                        }
                    }
                } else if self.flk {
                    r = lift(255, m.snap[0]);
                    self.flk = false;
                } else {
                    r = lift(255, m.snap[0]);
                    g = lift(220, m.snap[1]);
                    self.flk = true;
                }
                cv.fill_polygon(&px, &py_, clampc([r, g, b]));

                let mut r = lift(115, m.snap[0]);
                let mut g = lift(170, m.snap[1]);
                let mut b = 0i32;
                if missedcp <= 0 {
                    if var17 <= 45 && missedcp != -2 && self.cntan == 0 {
                        r = (r * var17 + m.csky[0] * (45 - var17)) / 45;
                        g = (g * var17 + m.csky[1] * (45 - var17)) / 45;
                        b = (b * var17 + m.csky[2] * (45 - var17)) / 45;
                    }
                } else if self.flk {
                    r = lift(255, m.snap[0]);
                    g = 0;
                }
                cv.draw_polygon(&px, &py_, clampc([r, g, b]));
            }
        } else {
            for i in 0..7 {
                px[i] = xs(px[i], pd[i], m);
                py_[i] = ys(py_[i], pd[i], m);
            }
            let r = lift(159, m.snap[0]);
            let g = lift(207, m.snap[1]);
            let b = lift(255, m.snap[2]);
            cv.fill_polygon(&px, &py_, clampc([r, g, b]));
            let r = lift(120, m.snap[0]);
            let g = lift(114, m.snap[1]);
            let b = lift(255, m.snap[2]);
            cv.draw_polygon(&px, &py_, clampc([r, g, b]));
        }
    }

    /// `xtGraphics.ctachm(mx, my, mouses, Control)` (xt:3706-3899): the
    /// menu click router, one branch per fase. `mouses` is the mouse
    /// state the shell threads through the loop (1 press, 2 release);
    /// presses light up the `pnext`/`pback`/`pcontin`/`pstar` pressed
    /// variants, releases replay the press as a keyboard key on the
    /// `Control` so each screen's own key branches route.
    pub fn ctachm(&mut self, xm: i32, ym: i32, mouses: i32, ctl: &mut Control) {
        if self.fase == 1 {
            if mouses == 1 {
                if self.over(&self.next[0], xm, ym, 440, 110) {
                    self.pnext = 1;
                }
                if self.over(&self.back[0], xm, ym, 50, 110) {
                    self.pback = 1;
                }
                if self.over(&self.contin1[0], xm, ym, 232, 170) {
                    self.pcontin = 1;
                }
            }
            if mouses == 2 {
                if self.pnext == 1 {
                    ctl.right = true;
                }
                if self.pback == 1 {
                    ctl.left = true;
                }
                if self.pcontin == 1 {
                    ctl.enter = true;
                }
            }
        }
        if self.fase == 3 {
            if mouses == 1 && self.over(&self.contin1[0], xm, ym, 232, 270) {
                self.pcontin = 1;
            }
            if mouses == 2 && self.pcontin == 1 {
                ctl.enter = true;
                self.pcontin = 0;
            }
        }
        if self.fase == 4 {
            if mouses == 1 && self.over(&self.back[0], xm, ym, 245, 320) {
                self.pback = 1;
            }
            if mouses == 2 && self.pback == 1 {
                ctl.enter = true;
                self.pback = 0;
            }
        }
        if self.fase == 6 {
            if mouses == 1 && self.over(&self.star[0], xm, ym, 234, 280) {
                self.pstar = 2;
            }
            if mouses == 2 && self.pstar == 2 {
                ctl.enter = true;
                self.pstar = 1;
            }
        }
        if self.fase == 7 {
            if mouses == 1 {
                if self.over(&self.next[0], xm, ym, 467, 276) {
                    self.pnext = 2;
                }
                if self.over(&self.back[0], xm, ym, 23, 276) {
                    self.pback = 2;
                }
                if self.over(&self.contin2[0], xm, ym, 232, 360) {
                    self.pcontin = 1;
                }
            }
            if mouses == 2 {
                if self.pnext == 2 {
                    ctl.right = true;
                }
                if self.pback == 2 {
                    ctl.left = true;
                }
                if self.pcontin == 1 {
                    ctl.enter = true;
                    self.pcontin = 0;
                }
            }
        }
        if self.fase == -5 {
            if mouses == 1 && self.over(&self.contin2[0], xm, ym, 230, 350 - self.pin) {
                self.pcontin = 1;
            }
            if mouses == 2 && self.pcontin == 1 {
                ctl.enter = true;
                self.pcontin = 0;
            }
        }
        if self.fase == -7 {
            if mouses == 1 {
                if self.overon(204, 143, 137, 22, xm, ym) {
                    self.opselect = 0;
                    self.shaded = true;
                }
                if self.overon(195, 171, 155, 22, xm, ym) {
                    self.opselect = 1;
                    self.shaded = true;
                }
                if self.overon(178, 197, 190, 22, xm, ym) {
                    self.opselect = 2;
                    self.shaded = true;
                }
                if self.overon(216, 223, 109, 22, xm, ym) {
                    self.opselect = 3;
                    self.shaded = true;
                }
            }
            if mouses == 2 && self.shaded {
                ctl.enter = true;
                self.shaded = false;
            }
        }
        if self.fase == 10 {
            if mouses == 1 {
                if self.overon(218, 246, 110, 22, xm, ym) {
                    self.opselect = 0;
                    self.shaded = true;
                }
                if self.overon(174, 275, 196, 22, xm, ym) {
                    self.opselect = 1;
                    self.shaded = true;
                }
                if self.overon(230, 306, 85, 22, xm, ym) {
                    self.opselect = 2;
                    self.shaded = true;
                }
            }
            if mouses == 2 && self.shaded {
                ctl.enter = true;
                self.shaded = false;
            }
        }
        if self.fase == 11 {
            if self.flipo == 1 || self.flipo == 3 {
                if mouses == 1 && self.over(&self.next[0], xm, ym, 460, 370) {
                    self.pnext = 1;
                }
                if mouses == 2 && self.pnext == 1 {
                    ctl.enter = true;
                    self.pnext = 0;
                }
            }
            if self.flipo == 5 {
                if mouses == 1 && self.over(&self.contin2[0], xm, ym, 230, 370) {
                    self.pcontin = 1;
                }
                if mouses == 2 && self.pcontin == 1 {
                    ctl.enter = true;
                    self.pcontin = 0;
                }
            }
        }
        if self.fase == 8 {
            if mouses == 1 && self.over(&self.next[0], xm, ym, 460, 370) {
                self.pnext = 1;
            }
            if mouses == 2 && self.pnext == 1 {
                ctl.enter = true;
                self.pnext = 0;
            }
        }
    }

    /// `xtGraphics.pauseimage(Image, Graphics)` (xt:1130-1178): freeze the
    /// live frame into `flexpix` and build the pause backdrop `fleximg`.
    /// The transform is a grayscale low-pass — the horizontal running
    /// average `(r+g+b + prev*30)/33` reset at each row start (`var7`
    /// column counter) — with a moving window tinted blue: once the
    /// pixel index passes `550*(106+var5)+156` the window's `var5`
    /// advances one row per 237 in-window pixels (`var4`), capping at
    /// 188 rows; those pixels become `((a+60)/3, (a+135)/3, (a+220)/3)`
    /// and the rest stay gray `(a,a,a)`. Rebuilds `fleximg` (the
    /// `MemoryImageSource` wrap; a copy here — every consumer rebuilds
    /// before drawing), blits it and resets `m.flex`.
    pub fn pauseimage(&mut self, cv: &mut Canvas, m: &mut Medium) {
        for i in 0..550 * 400 {
            let p = &cv.px[i * 3..i * 3 + 3];
            self.flexpix[i] =
                0xFF00_0000 | ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32;
        }
        let mut var4 = 0i32; // in-window pixels on the current window row
        let mut var5 = 0i32; // the window's row offset
        let mut var6 = 0i32; // the previous pixel's average
        let mut var7 = 0i32; // the column counter
        let mut var8 = 0usize;
        loop {
            let px = self.flexpix[var8];
            let r = ((px >> 16) & 0xFF) as i32;
            let g = ((px >> 8) & 0xFF) as i32;
            let b = (px & 0xFF) as i32;
            let var10;
            if var7 == 0 {
                var10 = (r + g + b) / 3;
                var6 = var10;
            } else {
                var10 = (r + g + b + var6 * 30) / 33;
                var6 = var10;
            }
            var7 += 1;
            if var7 == 550 {
                var7 = 0;
            }
            let out = if (var8 as i32) > 550 * (106 + var5) + 156 && var5 < 188 {
                let c = [(var10 + 60) / 3, (var10 + 135) / 3, (var10 + 220) / 3];
                var4 += 1;
                if var4 == 237 {
                    var5 += 1;
                    var4 = 0;
                }
                c
            } else {
                [var10, var10, var10]
            };
            self.flexpix[var8] =
                0xFF00_0000 | ((out[0] as u32) << 16) | ((out[1] as u32) << 8) | out[2] as u32;
            var8 += 1;
            if var8 >= 550 * 400 {
                break;
            }
        }
        self.fleximg = Gif {
            w: 550,
            h: 400,
            px: self.flexpix.clone(),
        };
        cv.draw_image(&self.fleximg, 0, 0);
        m.flex = 0;
    }

    /// `xtGraphics.fleximage(Image, Graphics, int)` (xt:371-430): the
    /// result-screen fade. `step == 0` re-grabs the live frame and
    /// re-hues every pixel to the 0.7 hue at its own saturation/value
    /// (JDK `RGBtoHSB`/`getHSBColor` math, see `rgb_to_hsb`/
    /// `hsb_to_rgb`); every call then darkens ~80% of the pixels
    /// (`m.random() > 0.2`) by the per-channel factors climbing with
    /// `step` (`flex_darken`) — the xorshift random stand-in means the
    /// mask differs from the JVM's stream (documented divergence; the
    /// arithmetic itself is JDK-pinned). Rebuilds `fleximg` and blits.
    pub fn fleximage(&mut self, cv: &mut Canvas, m: &mut Medium, step: i32) {
        if step == 0 {
            for i in 0..550 * 400 {
                let p = &cv.px[i * 3..i * 3 + 3];
                self.flexpix[i] =
                    0xFF00_0000 | ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32;
            }
            for i in 0..550 * 400 {
                let px = self.flexpix[i];
                let r = ((px >> 16) & 0xFF) as i32;
                let g = ((px >> 8) & 0xFF) as i32;
                let b = (px & 0xFF) as i32;
                let (_h, s, v) = rgb_to_hsb(r, g, b);
                let (nr, ng, nb) = hsb_to_rgb(0.7f32, s, v);
                self.flexpix[i] =
                    0xFF00_0000 | ((nr as u32) << 16) | ((ng as u32) << 8) | nb as u32;
            }
        }
        for i in 0..550 * 400 {
            if (m.random() as f64) > 0.2 {
                let px = self.flexpix[i];
                let r = ((px >> 16) & 0xFF) as i32;
                let g = ((px >> 8) & 0xFF) as i32;
                let b = (px & 0xFF) as i32;
                let (nr, ng, nb) = flex_darken(r, g, b, step);
                self.flexpix[i] =
                    0xFF00_0000 | ((nr as u32) << 16) | ((ng as u32) << 8) | nb as u32;
            }
        }
        self.fleximg = Gif {
            w: 550,
            h: 400,
            px: self.flexpix.clone(),
        };
        cv.draw_image(&self.fleximg, 0, 0);
    }

    /// `xtGraphics.pausedgame(Graphics, int stage, Control, Record)`
    /// (xt:1342-1451): the in-race pause menu over the frozen
    /// `fleximg` backdrop. Up/down cycle `opselect` 0..3 with the
    /// backdrop re-blitted to erase the previous slot (also on the
    /// `shaded` hover state), one round-rect slot is drawn at a time
    /// (`fillRoundRect` (64,143,223); stroke (0,89,223) or, while
    /// shaded, (225,200,255)), then the `paused` plate at (156,106).
    /// enter/handb dispatch: resume → fase 0; replay → fase -1 when the
    /// recording has >= 300 frames (else the cantreply notice, -8);
    /// instructions arm `oldfase = -7` and go to fase 11; quit returns
    /// to the main menu (fase 10, opselect reset). The resume arms read
    /// `loadedt[stage - 1] && !mutem` exactly like the source
    /// (xt:1419/1428).
    pub fn pausedgame(&mut self, cv: &mut Canvas, stage: i32, ctl: &mut Control, rpd: &Record) {
        if ctl.up {
            cv.draw_image(&self.fleximg, 0, 0);
            self.opselect += -1;
            if self.opselect == -1 {
                self.opselect = 3;
            }
            ctl.up = false;
        }
        if ctl.down {
            cv.draw_image(&self.fleximg, 0, 0);
            self.opselect += 1;
            if self.opselect == 4 {
                self.opselect = 0;
            }
            ctl.down = false;
        }
        if self.shaded {
            cv.draw_image(&self.fleximg, 0, 0);
        }
        // The four slots; only the selected one paints (the others were
        // erased by the backdrop re-blit above).
        let slot = match self.opselect {
            0 => Some((204, 143, 137, 22)),
            1 => Some((195, 171, 155, 22)),
            2 => Some((178, 197, 190, 22)),
            3 => Some((216, 223, 109, 22)),
            _ => None,
        };
        if let Some((x, y, w, h)) = slot {
            cv.fill_round_rect(x, y, w, h, 7, 20, [64, 143, 223]);
            let stroke = if self.shaded {
                [225, 200, 255]
            } else {
                [0, 89, 223]
            };
            cv.draw_round_rect(x, y, w, h, 7, 20, stroke);
        }
        cv.draw_image(&self.paused, 156, 106);
        if ctl.enter || ctl.handb {
            if self.opselect == 0 {
                if self.loadedt[(stage - 1) as usize]
                    && !self.mutem
                    && let Some(r) = self.stracks[(stage - 1) as usize].as_mut()
                {
                    r.resume();
                }
                self.fase = 0;
            }
            if self.opselect == 1 {
                if rpd.caught >= 300 {
                    if self.loadedt[(stage - 1) as usize]
                        && !self.mutem
                        && let Some(r) = self.stracks[(stage - 1) as usize].as_mut()
                    {
                        r.resume();
                    }
                    self.fase = -1;
                } else {
                    self.fase = -8;
                }
            }
            if self.opselect == 2 {
                self.oldfase = -7;
                self.fase = 11;
            }
            if self.opselect == 3 {
                self.fase = 10;
                self.opselect = 0;
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
        armed_stage1, dump_masked, hex, loaded_xt, pattern_px, rect_ink, stage1_m,
    };

    /// The deterministic procedural frame both pause pins share:
    /// `r = (x*7 + y*3) & 255`, `g = (x*13) & 255`, `b = (y*11) & 255`.
    fn pattern_canvas() -> Canvas {
        let mut cv = Canvas::new(550, 400);
        for i in 0..550 * 400 {
            let x = (i % 550) as i32;
            let y = (i / 550) as i32;
            cv.px[i * 3] = ((x * 7 + y * 3) & 255) as u8;
            cv.px[i * 3 + 1] = ((x * 13) & 255) as u8;
            cv.px[i * 3 + 2] = ((y * 11) & 255) as u8;
        }
        cv
    }

    #[test]
    fn stat_draws_the_full_hud_panels_and_text() {
        let mut xt = loaded_xt();
        let mut m = stage1_m();
        xt.snap(1, &m);
        let mut cp = CheckPoints::new();
        let mut mad = Madness::new(0);
        let mut ctl = Control::new();
        let mut cv = Canvas::new(550, 400);
        xt.resetstat(1);
        xt.starcnt = 0; // the countdown over: the HUD draws
        xt.stat(&mut mad, &mut cp, &mut ctl, true, &mut cv, &mut m);

        assert!(
            rect_ink(&cv, 360, 7, 478, 27) > 800,
            "the damage+power panels"
        );
        assert!(rect_ink(&cv, 19, 7, 90, 27) > 200, "the lap panel");
        assert!(rect_ink(&cv, 92, 7, 150, 27) > 200, "the wasted panel");
        assert!(rect_ink(&cv, 42, 27, 110, 55) > 200, "the position panel");
        assert!(rect_ink(&cv, 51, 11, 90, 19) > 10, "the lap counter text");
        assert!(
            rect_ink(&cv, 150, 11, 185, 19) > 10,
            "the wasted counter text"
        );
        assert!(rect_ink(&cv, 422, 11, 521, 20) > 60, "the damage bar");
        assert!(rect_ink(&cv, 422, 31, 470, 40) > 60, "the power bar");
        assert_eq!(m.flex, 1, "the full draw advanced the flex machine");
        // the arrow's fresh rig keeps a zero bearing: below the 7 gate
        assert_eq!(rect_ink(&cv, 240, 320, 320, 360), 0, "no arrow yet");
    }

    #[test]
    fn stat_won_screen_then_enter_flips_fase_minus_2() {
        let mut xt = loaded_xt();
        let mut m = stage1_m();
        xt.snap(1, &m);
        let mut cp = CheckPoints::new();
        cp.nlaps = 3;
        cp.nsp = 12;
        cp.clear[0] = cp.nlaps * cp.nsp;
        cp.pos[0] = 0;
        let mut mad = Madness::new(0);
        let mut ctl = Control::new();
        let mut cv = Canvas::new(550, 400);
        xt.resetstat(1);
        xt.starcnt = 0;
        xt.fase = 0; // the in-race context (the shell's fase 0 racing run)

        xt.stat(&mut mad, &mut cp, &mut ctl, true, &mut cv, &mut m);
        assert_eq!(xt.holdcnt, 1, "the win screen counts the hold");
        assert!(cp.haltall, "the race halted");
        assert!(xt.winner);
        assert_eq!(xt.fase, 0, "no transition without the press");

        ctl.enter = true;
        xt.stat(&mut mad, &mut cp, &mut ctl, true, &mut cv, &mut m);
        assert_eq!(xt.fase, -2, "the continue armed the crash-cam pass");
        assert!(!ctl.enter, "the press consumed");
    }

    #[test]
    fn stat_drawcs_shadows_and_the_arrow_car_banner() {
        // mode 0 lifts the color through the snap; mode 1 lays the same
        // glyphs at +1/+1 in black first (the shadow), then the main pass.
        // The "A" cell is the atlas coverage map at 11pt (the race HUD size).
        let mut xt = loaded_xt();
        let m = stage1_m();
        let mut cv0 = Canvas::new(550, 400);
        let mut cv1 = Canvas::new(550, 400);
        let mut cv2 = Canvas::new(550, 400);
        cv1.fill([255, 255, 255]);
        cv2.fill([255, 255, 255]);
        xt.drawcs(&mut cv0, 11, 105, "A", [255, 128, 0], 0, &m);
        xt.drawcs(&mut cv1, 11, 105, "A", [255, 128, 0], 1, &m);
        xt.drawcs(&mut cv2, 11, 105, "A", [255, 128, 0], 2, &m);

        let a = crate::xtfont::atlas();
        let size = 11;
        let w = crate::xtfont::string_width("A", size);
        let x = 275 - w / 2;
        let at = |cv: &Canvas, x: i32, y: i32| {
            let i = (y as usize * cv.w + x as usize) * 3;
            [cv.px[i], cv.px[i + 1], cv.px[i + 2]]
        };
        fn mul8(a: u8, b: u8) -> u16 {
            (a as u16 * b as u16 + 127) / 255
        }
        let mix = |cov: u8, ink: [u8; 3], bg: [u8; 3]| {
            [
                (mul8(cov, ink[0]) + mul8(255 - cov, bg[0])).min(255) as u8,
                (mul8(cov, ink[1]) + mul8(255 - cov, bg[1])).min(255) as u8,
                (mul8(cov, ink[2]) + mul8(255 - cov, bg[2])).min(255) as u8,
            ]
        };

        // mode 0 over black = MUL8(coverage, lifted)
        let lifted = clampc([lift(255, -5), lift(128, -5), lift(0, 20)]);
        let cell = a.cell(size, b'A' as u32);
        let ox = x - a.glyph_dx;
        let oy = 105 - a.baseline;
        let mut main_ink = 0;
        let mut opaque = 0;
        for cy in 0..a.cell_h {
            for cx in 0..a.cell_w {
                let cov = cell[cy * a.cell_w + cx];
                if cov == 0 {
                    continue;
                }
                main_ink += 1;
                let px = (ox + cx as i32, oy + cy as i32);
                assert_eq!(
                    at(&cv0, px.0, px.1),
                    mix(cov, lifted, [0, 0, 0]),
                    "mode 0 cell ({cx},{cy})"
                );
                if cov == 255 {
                    opaque += 1;
                    assert_eq!(at(&cv1, px.0, px.1), lifted, "mode 1's opaque main pass");
                }
            }
        }
        assert!(main_ink > 20 && opaque > 15, "A inked {main_ink}/{opaque}");

        // the +1/+1 black shadow shows where the main cell is empty but the
        // shifted cell inks: black over the white backdrop = 255 - coverage
        let shadow_cell = a.cell(size, b'A' as u32);
        let (mut sx, mut sy, mut scov) = (0, 0, 0);
        'find: for cy in 0..a.cell_h {
            for cx in 0..a.cell_w {
                let cov = shadow_cell[cy * a.cell_w + cx];
                // the main cell's coverage at this canvas pixel: the shadow
                // cell is shifted +1/+1, so main column/row are cx+1/cy+1
                let mcx = cx as i32 + 1;
                let mcy = cy as i32 + 1;
                let main_cov = if (mcx as usize) < a.cell_w && (mcy as usize) < a.cell_h {
                    shadow_cell[mcy as usize * a.cell_w + mcx as usize]
                } else {
                    0
                };
                if cov > 128 && main_cov == 0 {
                    sx = ox + cx as i32 + 1;
                    sy = oy + cy as i32 + 1;
                    scov = cov;
                    break 'find;
                }
            }
        }
        assert!(scov > 0, "found a shadow-only pixel");
        assert_eq!(
            at(&cv1, sx, sy),
            [255 - scov, 255 - scov, 255 - scov],
            "the +1/+1 black shadow over white"
        );

        // mode 2's shadow blends the lifted color with the sky 1:2
        let shadow = clampc([
            (lift(255, -5) + m.csky[0] * 2) / 3,
            (lift(128, -5) + m.csky[1] * 2) / 3,
            (lift(0, 20) + m.csky[2] * 2) / 3,
        ]);
        assert_eq!(
            at(&cv2, sx, sy),
            mix(scov, shadow, [255, 255, 255]),
            "the sky-tinted shadow"
        );

        // the car banner drawcs pair draws the bracket line + name
        let mut cv3 = Canvas::new(550, 400);
        cv3.fill([255, 255, 255]);
        let cp = CheckPoints::new();
        xt.arrace = true;
        xt.sc[1] = 2;
        xt.ana = 40;
        xt.arrow(&mut cv3, 0, 0, &cp, true, &m);
        assert!(rect_ink(&cv3, 190, 0, 380, 16) > 50, "the car banner text");
    }

    // ────────────── the loading screen slice ──────────────

    /// `pausedgame`'s resume routes (xt:1419/1428): opselect 0 and the
    /// caught>=300 replay arm resume the armed track; short replays and
    /// `mutem` do not.
    #[test]
    fn pausedgame_resume_routes() {
        let mut xt = armed_stage1();
        let mut cv = Canvas::new(550, 400);
        xt.fleximg = Gif {
            w: 550,
            h: 400,
            px: pattern_px(),
        };
        let mut rpd = Record::new();
        let mut ctl = Control::new();

        // opselect 0 + enter: resume + fase 0
        xt.stracks[0].as_mut().unwrap().stop();
        xt.opselect = 0;
        ctl.enter = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.fase, 0);
        assert!(xt.stracks[0].as_ref().unwrap().playing);

        // opselect 1 + long replay: resume + fase -1
        xt.stracks[0].as_mut().unwrap().stop();
        xt.opselect = 1;
        rpd.caught = 300;
        ctl.enter = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.fase, -1);
        assert!(xt.stracks[0].as_ref().unwrap().playing);

        // opselect 1 + short replay: fase -8, no resume
        xt.opselect = 1;
        rpd.caught = 299;
        ctl.enter = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.fase, -8);

        // mutem blocks the resume (fase still advances)
        xt.stracks[0].as_mut().unwrap().stop();
        xt.opselect = 0;
        xt.mutem = true;
        ctl.enter = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.fase, 0);
        assert!(!xt.stracks[0].as_ref().unwrap().playing);
    }

    /// `stat`'s fase -2 continue (xt:1617) stops the armed track.
    #[test]
    fn stat_minus2_stops_the_track() {
        let mut xt = armed_stage1();
        let mut mad = Madness::new(0);
        let mut cp = CheckPoints::new();
        cp.stage = 1;
        let mut ctl = Control::new();
        ctl.enter = true;
        let mut m = Medium::new();
        let mut cv = Canvas::new(550, 400);
        xt.stat(&mut mad, &mut cp, &mut ctl, false, &mut cv, &mut m);
        assert_eq!(xt.fase, -6);
        assert!(!xt.stracks[0].as_ref().unwrap().playing);
    }

    /// `stat`'s sound triggers (xt:1718-1742, 1987, 2049): the starcnt-35
    /// `three`, the stunt `powerup` once `auscnt` drops, and the
    /// checkpoint sting on a fresh clear.
    #[test]
    fn stat_countdown_stunt_and_checkpoint_triggers() {
        let mut xt = loaded_xt();
        let mut m = Medium::new();
        let mut mad = Madness::new(0);
        mad.trcnt = 10;
        mad.travzy = 300;
        mad.clear = 1;
        let mut cp = CheckPoints::new();
        let mut ctl = Control::new();
        let mut cv = Canvas::new(550, 400);
        xt.resetstat(1);
        xt.starcnt = 35;
        xt.stat(&mut mad, &mut cp, &mut ctl, true, &mut cv, &mut m);
        assert!(xt.three.as_ref().unwrap().playing, "starcnt 35");
        assert!(xt.powerup.as_ref().unwrap().playing, "stunt auscnt");
        assert!(
            xt.checkpoint.as_ref().unwrap().playing,
            "fresh checkpoint clear"
        );
    }

    /// `pauseimage` (xt:1130-1178) against JDK 1.8.0_40 (scratch
    /// `PauseGt`): the whole transformed frame — the horizontal
    /// low-pass average, the moving blue window with its 237-pixel /
    /// 188-row counters, the rebuild and blit — byte-exact.
    #[test]
    fn pauseimage_matches_the_jdk() {
        let mut xt = loaded_xt();
        let mut m = Medium::new();
        m.flex = 2;
        let mut cv = pattern_canvas();
        xt.pauseimage(&mut cv, &mut m);
        assert_eq!(m.flex, 0, "the flex machine reset");
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[]))),
            "906623afd63f02cc48b6fdcc4491802c",
            "pauseimage transform"
        );
    }

    /// `pausedgame` over the pause backdrop against JDK 1.8.0_40
    /// (scratch `PauseGt`): the default opselect-1 slot, un-shaded
    /// stroke, and the `paused` plate — full-frame byte-exact.
    #[test]
    fn pausedgame_matches_the_jdk_over_the_pause_backdrop() {
        let mut xt = loaded_xt();
        let mut m = Medium::new();
        let mut cv = pattern_canvas();
        xt.pauseimage(&mut cv, &mut m);
        let mut ctl = Control::new();
        let rpd = Record::new();
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(
            hex(&md5(&dump_masked(&cv, &[]))),
            "981651e09bf250c19f6e3319b4278329",
            "the opselect-1 slot + the paused plate"
        );
    }

    /// The flex math against JDK 1.8.0_40 probes (scratch `PauseGt`):
    /// `RGBtoHSB` → `getHSBColor(0.7, s, v)` (the fleximage re-hue pass;
    /// the equal-channel rows pass through untouched, red/green/blue all
    /// land on the (51,0,255) pure-hue ray) and the darkening
    /// arithmetic at several steps (the clamp rows included).
    #[test]
    fn flex_math_matches_the_jvm() {
        let rows = [
            ((0, 0, 0), (0, 0, 0)),
            ((255, 255, 255), (255, 255, 255)),
            ((255, 0, 0), (51, 0, 255)),
            ((0, 255, 0), (51, 0, 255)),
            ((0, 0, 255), (51, 0, 255)),
            ((128, 64, 32), (51, 32, 128)),
            ((10, 200, 90), (48, 10, 200)),
            ((253, 1, 128), (51, 1, 253)),
            ((77, 77, 77), (77, 77, 77)),
            ((1, 2, 3), (1, 1, 3)),
        ];
        for ((r, g, b), want) in rows {
            let (_h, s, v) = rgb_to_hsb(r, g, b);
            assert_eq!(hsb_to_rgb(0.7f32, s, v), want, "rgb {r},{g},{b}");
        }
        let dark = [
            ((255, 255, 255, 0), (153, 165, 191)),
            ((255, 255, 255, 5), (153, 165, 148)),
            ((100, 50, 200, 3), (60, 32, 129)),
            ((10, 10, 10, 0), (6, 6, 7)),
            ((240, 100, 30, 6), (144, 65, 16)),
            ((0, 0, 0, 3), (0, 0, 0)),
            ((225, 225, 225, 30), (135, 146, 0)),
            ((127, 63, 31, 1), (76, 40, 22)),
        ];
        for ((r, g, b, step), want) in dark {
            assert_eq!(
                flex_darken(r, g, b, step),
                want,
                "rgb {r},{g},{b} step {step}"
            );
        }
    }

    /// `pausedgame`'s slot machine beyond the pin: up/down cycle
    /// `opselect` 0..3 with the backdrop re-blit erasing the previous
    /// slot, the highlight stroke follows the `shaded` hover, and the
    /// four enter routes dispatch (resume, replay by recording length,
    /// instructions with `oldfase = -7`, quit with the selection reset).
    #[test]
    fn pausedgame_slots_and_enter_routes() {
        let mut xt = loaded_xt();
        let mut m = Medium::new();
        let mut cv = pattern_canvas();
        xt.pauseimage(&mut cv, &mut m);
        let mut ctl = Control::new();
        let mut rpd = Record::new();
        let at = |cv: &Canvas, x: i32, y: i32| {
            let i = (y as usize * cv.w + x as usize) * 3;
            [cv.px[i], cv.px[i + 1], cv.px[i + 2]]
        };

        // default opselect 1, un-shaded stroke on the top edge
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(at(&cv, 272, 171), [0, 89, 223], "slot-1 stroke");

        // the hover stroke
        xt.shaded = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(at(&cv, 272, 171), [225, 200, 255], "the shaded stroke");
        xt.shaded = false;

        // down moves to slot 2 and erases slot 1 back to the backdrop
        ctl.down = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.opselect, 2);
        assert!(!ctl.down);
        assert_eq!(at(&cv, 273, 197), [0, 89, 223], "slot-2 stroke");
        let fp = xt.fleximg.px[171 * 550 + 272];
        assert_eq!(
            at(&cv, 272, 171),
            [
                ((fp >> 16) & 0xFF) as u8,
                ((fp >> 8) & 0xFF) as u8,
                (fp & 0xFF) as u8
            ],
            "slot 1 erased to the backdrop"
        );

        // wraps: 2 → 3 → 0 → up → 3
        ctl.down = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.opselect, 3);
        ctl.down = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.opselect, 0, "down wraps past 3");
        ctl.up = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.opselect, 3, "up wraps past 0");

        // resume → the race
        xt.opselect = 0;
        ctl.enter = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.fase, 0);
        assert!(!ctl.enter);

        // replay under 300 recorded frames → the cantreply notice
        xt.opselect = 1;
        rpd.caught = 299;
        ctl.handb = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.fase, -8);
        assert!(!ctl.handb);

        // enough footage → the replay film
        rpd.caught = 300;
        ctl.enter = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.fase, -1);

        // instructions arm the pause return
        xt.opselect = 2;
        ctl.enter = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.oldfase, -7);
        assert_eq!(xt.fase, 11);

        // quit to the menu resets the selection
        xt.opselect = 3;
        ctl.enter = true;
        xt.pausedgame(&mut cv, 1, &mut ctl, &rpd);
        assert_eq!(xt.fase, 10);
        assert_eq!(xt.opselect, 0);
    }

    // ────────────── the carselect slice ──────────────

    /// `ctachm`'s fase-10 click routing: the press lights the hovered
    /// slot and sets `shaded`, the release replays it as an enter key and
    /// clears the shade; presses outside the slots stay inert.
    #[test]
    fn ctachm_routes_the_main_menu_click() {
        let mut xt = loaded_xt();
        let mut ctl = Control::new();
        xt.fase = 10;

        // press over the New Game slot (218,246,110,22)
        xt.ctachm(250, 250, 1, &mut ctl);
        assert_eq!(xt.opselect, 0);
        assert!(xt.shaded, "the hover lit the slot");

        // release → enter key for maini, shade cleared
        xt.ctachm(250, 250, 2, &mut ctl);
        assert!(ctl.enter, "the release synthesised the key");
        assert!(!xt.shaded);

        // a press outside every slot stays inert
        ctl.enter = false;
        xt.ctachm(10, 10, 1, &mut ctl);
        assert!(!xt.shaded, "no slot hit");
        xt.ctachm(10, 10, 2, &mut ctl);
        assert!(!ctl.enter, "no release replay without a press");
    }

    /// `ctachm`'s image-slot routing across the other fases: every
    /// press lights the pressed variant (`pnext`/`pback`/`pcontin`/
    /// `pstar`), every release replays it as the screen's key and
    /// clears the flag (the `-5`/`-7`/`10` arms clear on press instead,
    /// like the source).
    #[test]
    fn ctachm_routes_the_image_slots() {
        let mut xt = loaded_xt();
        let mut ctl = Control::new();

        // fase 1: next → right, back → left, contin1 → enter
        xt.fase = 1;
        xt.pnext = 0;
        xt.ctachm(450, 115, 1, &mut ctl);
        assert_eq!(xt.pnext, 1);
        xt.ctachm(450, 115, 2, &mut ctl);
        assert!(ctl.right);
        assert_eq!(xt.pnext, 1, "the fase-1 arm keeps the flag");
        ctl.right = false;
        xt.ctachm(55, 115, 1, &mut ctl);
        assert_eq!(xt.pback, 1);
        xt.ctachm(55, 115, 2, &mut ctl);
        assert!(ctl.left);
        ctl.left = false;
        xt.ctachm(240, 175, 1, &mut ctl);
        assert_eq!(xt.pcontin, 1);
        xt.ctachm(240, 175, 2, &mut ctl);
        assert!(ctl.enter);
        ctl.enter = false;

        // fase 3: continue1 → enter with the flag cleared
        xt.fase = 3;
        xt.ctachm(240, 275, 1, &mut ctl);
        assert_eq!(xt.pcontin, 1);
        xt.ctachm(240, 275, 2, &mut ctl);
        assert!(ctl.enter);
        assert_eq!(xt.pcontin, 0);
        ctl.enter = false;

        // fase 4: back → enter with the flag cleared
        xt.fase = 4;
        xt.ctachm(250, 325, 1, &mut ctl);
        assert_eq!(xt.pback, 1);
        xt.ctachm(250, 325, 2, &mut ctl);
        assert!(ctl.enter);
        assert_eq!(xt.pback, 0);
        ctl.enter = false;

        // fase 6: star → enter, pstar 2 → 1 (the star slots fill at
        // snap() time — the stage palette pass — not at loadpak)
        let m = Medium::new();
        xt.snap(1, &m);
        assert!(xt.star[0].w > 0, "the snapped star slot");
        xt.fase = 6;
        xt.ctachm(240, 285, 1, &mut ctl);
        assert_eq!(xt.pstar, 2);
        xt.ctachm(240, 285, 2, &mut ctl);
        assert!(ctl.enter);
        assert_eq!(xt.pstar, 1);
        ctl.enter = false;

        // fase 7: next → right (pnext 2), back → left (pback 2),
        // contin2 → enter (cleared)
        xt.fase = 7;
        xt.ctachm(480, 285, 1, &mut ctl);
        assert_eq!(xt.pnext, 2);
        xt.ctachm(480, 285, 2, &mut ctl);
        assert!(ctl.right);
        ctl.right = false;
        xt.ctachm(40, 285, 1, &mut ctl);
        assert_eq!(xt.pback, 2);
        xt.ctachm(40, 285, 2, &mut ctl);
        assert!(ctl.left);
        ctl.left = false;
        xt.ctachm(240, 365, 1, &mut ctl);
        assert_eq!(xt.pcontin, 1);
        xt.ctachm(240, 365, 2, &mut ctl);
        assert!(ctl.enter);
        assert_eq!(xt.pcontin, 0);
        ctl.enter = false;

        // fase -5: contin2 at (230, 350 - pin)
        xt.fase = -5;
        xt.pin = 60;
        xt.ctachm(240, 295, 1, &mut ctl);
        assert_eq!(xt.pcontin, 1);
        xt.ctachm(240, 295, 2, &mut ctl);
        assert!(ctl.enter);
        assert_eq!(xt.pcontin, 0);
        ctl.enter = false;

        // fase 11 flipo 1: next → enter; flipo 5: contin2 → enter
        xt.fase = 11;
        xt.flipo = 1;
        xt.ctachm(470, 375, 1, &mut ctl);
        assert_eq!(xt.pnext, 1);
        xt.ctachm(470, 375, 2, &mut ctl);
        assert!(ctl.enter);
        assert_eq!(xt.pnext, 0);
        ctl.enter = false;
        xt.flipo = 5;
        xt.ctachm(240, 375, 1, &mut ctl);
        assert_eq!(xt.pcontin, 1);
        xt.ctachm(240, 375, 2, &mut ctl);
        assert!(ctl.enter);
        ctl.enter = false;

        // fase 8: next → enter
        xt.fase = 8;
        xt.ctachm(470, 375, 1, &mut ctl);
        assert_eq!(xt.pnext, 1);
        xt.ctachm(470, 375, 2, &mut ctl);
        assert!(ctl.enter);
        assert_eq!(xt.pnext, 0);
        ctl.enter = false;

        // fase -7: the in-race pause rects (overon, exact)
        xt.fase = -7;
        xt.ctachm(250, 150, 1, &mut ctl);
        assert_eq!(xt.opselect, 0);
        assert!(xt.shaded);
        xt.ctachm(250, 150, 2, &mut ctl);
        assert!(ctl.enter);
        assert!(!xt.shaded);
    }

    // ────────────── the shared dump helpers ──────────────
}
