//! The audio-control slice: the voice ladder (`playsounds`, `sparkeng`,
//! `crash`, `skid`, `stopairs`), the lifecycle teardown (`stopallnow`) and
//! the mix-bus binding (`bind_audio`).

use super::*;

impl XtGraphics {
    /// `xtGraphics.playsounds(Madness, Control, int)` (xt:757-933) — the
    /// audio hub: the engine/skid/air/wasted voice ladder (xt:757-880),
    /// the per-tick counter decay (xt:883-897), then the fase-gated
    /// mutes/mutem sync (xt:902-917) and the per-iteration tail
    /// (newcar reset, cntwis). `m` threads through for the `random()`
    /// picks (Java `this.m`).
    pub fn playsounds(&mut self, mad: &Madness, ctl: &Control, stage: i32, m: &mut Medium) {
        // xt:757-880 — the engine hub; gated on the live race.
        if self.fase == 0 && self.starcnt < 35 && self.cntwis != 8 && !self.mutes {
            let mut var4 = ctl.up && mad.speed > 0.0 || ctl.down && mad.speed < 10.0;
            let mut var5 = mad.skid == 1 && ctl.handb
                || (mad.scz[0] - (mad.scz[1] + mad.scz[0] + mad.scz[2] + mad.scz[3]) / 4.0).abs()
                    > 1.0
                || (mad.scx[0] - (mad.scx[1] + mad.scx[0] + mad.scx[2] + mad.scx[3]) / 4.0).abs()
                    > 1.0;
            let mut var6 = false;
            if ctl.up && mad.speed < 10.0 {
                var5 = true;
                var4 = true;
                var6 = true;
            }

            if var4 && mad.mtouch {
                if !mad.capsized {
                    if !var5 {
                        if mad.power != 98.0 {
                            if mad.speed.abs() > 0.0 && mad.speed.abs() <= SWITS[mad.cn][0] as f32 {
                                let mut var7 =
                                    (3.0 * mad.speed.abs() / SWITS[mad.cn][0] as f32) as i32;
                                if var7 == 2 {
                                    if self.pwait == 0 {
                                        var7 = 0;
                                    } else {
                                        self.pwait -= 1;
                                    }
                                } else {
                                    self.pwait = 7;
                                }
                                self.sparkeng(var7);
                            }

                            if mad.speed.abs() > SWITS[mad.cn][0] as f32
                                && mad.speed.abs() <= SWITS[mad.cn][1] as f32
                            {
                                let mut var8 = (3.0 * (mad.speed.abs() - SWITS[mad.cn][0] as f32)
                                    / (SWITS[mad.cn][1] - SWITS[mad.cn][0]) as f32)
                                    as i32;
                                if var8 == 2 {
                                    if self.pwait == 0 {
                                        var8 = 0;
                                    } else {
                                        self.pwait -= 1;
                                    }
                                } else {
                                    self.pwait = 7;
                                }
                                self.sparkeng(var8);
                            }

                            if mad.speed.abs() > SWITS[mad.cn][1] as f32
                                && mad.speed.abs() <= SWITS[mad.cn][2] as f32
                            {
                                let var9 = (3.0 * (mad.speed.abs() - SWITS[mad.cn][1] as f32)
                                    / (SWITS[mad.cn][2] - SWITS[mad.cn][1]) as f32)
                                    as i32;
                                self.sparkeng(var9);
                            }
                        } else {
                            let mut var10 = 2;
                            if self.pwait == 0 {
                                if mad.speed.abs() > SWITS[mad.cn][1] as f32 {
                                    var10 = 3;
                                }
                            } else {
                                self.pwait -= 1;
                            }
                            self.sparkeng(var10);
                        }
                    } else {
                        self.sparkeng(-1);
                        if var6 {
                            if self.stopcnt <= 0 {
                                if let Some(a) = self.air[5].as_mut() {
                                    a.loop_();
                                }
                                self.stopcnt = 10;
                            }
                        } else if self.stopcnt <= -2 {
                            let pick = 2 + (m.random() * 3.0) as usize;
                            if let Some(a) = self.air[pick].as_mut() {
                                a.loop_();
                            }
                            self.stopcnt = 7;
                        }
                    }
                } else {
                    self.sparkeng(3);
                }

                self.grrd = false;
                self.aird = false;
            } else {
                self.pwait = 15;
                if !mad.mtouch && !self.grrd && m.random() as f64 > 0.4 {
                    let pick = (m.random() * 4.0) as usize;
                    if let Some(a) = self.air[pick].as_mut() {
                        a.loop_();
                    }
                    self.stopcnt = 5;
                    self.grrd = true;
                }

                if !mad.wtouch && !self.aird {
                    self.stopairs();
                    let pick = (m.random() * 4.0) as usize;
                    if let Some(a) = self.air[pick].as_mut() {
                        a.loop_();
                    }
                    self.stopcnt = 10;
                    self.aird = true;
                }

                self.sparkeng(-1);
            }

            if mad.cntdest != 0 && self.cntwis < 7 {
                if !self.pwastd {
                    if let Some(w) = self.wastd.as_mut() {
                        w.loop_();
                    }
                    self.pwastd = true;
                }
            } else {
                if self.pwastd {
                    if let Some(w) = self.wastd.as_mut() {
                        w.stop();
                    }
                    self.pwastd = false;
                }

                if self.cntwis == 7
                    && !self.mutes
                    && let Some(f) = self.firewasted.as_mut()
                {
                    f.play();
                }
            }
        } else {
            self.sparkeng(-2);
            if self.pwastd {
                if let Some(w) = self.wastd.as_mut() {
                    w.stop();
                }
                self.pwastd = false;
            }
        }

        // xt:883-897 — the per-tick counter decay.
        if self.stopcnt != -20 {
            if self.stopcnt == 1 {
                self.stopairs();
            }
            self.stopcnt -= 1;
        }

        if self.bfcrash != 0 {
            self.bfcrash -= 1;
        }

        if self.bfskid != 0 {
            self.bfskid -= 1;
        }

        if mad.newcar {
            self.cntwis = 0;
        }

        // xt:902-917 — the mute toggles only run in the race phases.
        if self.fase == 0
            || self.fase == 6
            || self.fase == -1
            || self.fase == -2
            || self.fase == -3
            || self.fase == -4
            || self.fase == -5
        {
            if self.mutes != ctl.mutes {
                self.mutes = ctl.mutes;
            }
            if ctl.mutem != self.mutem {
                self.mutem = ctl.mutem;
                let idx = (stage - 1) as usize;
                if self.mutem {
                    if self.loadedt[idx]
                        && let Some(r) = self.stracks[idx].as_mut()
                    {
                        r.stop();
                    }
                } else if self.loadedt[idx]
                    && let Some(r) = self.stracks[idx].as_mut()
                {
                    r.resume();
                }
            }
        }

        if mad.cntdest != 0 && self.cntwis < 7 {
            if mad.dest {
                self.cntwis += 1;
            }
        } else {
            if mad.cntdest == 0 {
                self.cntwis = 0;
            }
            if self.cntwis == 7 {
                self.cntwis = 8;
            }
        }
    }

    /// `xtGraphics.sparkeng(int)` (xt:2385-2399): the engine-voice
    /// ladder — voice `var1 + 1` loops and every other armed voice
    /// stops (`sc[0]` picks the car's row of `enginsignature`).
    /// `sparkeng(-2)` silences all five; `sparkeng(3)` runs the top.
    pub fn sparkeng(&mut self, var1: i32) {
        let var1 = var1 + 1;
        for var2 in 0..5 {
            if var1 == var2 {
                if !self.pengs[var2 as usize] {
                    let row = self.enginsignature[self.sc[0] as usize][var2 as usize] as usize;
                    if let Some(e) = self.engs[row][var2 as usize].as_mut() {
                        e.loop_();
                    }
                    self.pengs[var2 as usize] = true;
                }
            } else if self.pengs[var2 as usize] {
                let row = self.enginsignature[self.sc[0] as usize][var2 as usize] as usize;
                if let Some(e) = self.engs[row][var2 as usize].as_mut() {
                    e.stop();
                }
                self.pengs[var2 as usize] = false;
            }
        }
    }

    /// `xtGraphics.crash(float, int)` (xt:935-1006): the crash-sound
    /// picker — `bfcrash` debounces; band 0 round-robins the
    /// lowcrash/crash banks (flipping direction at the ends via
    /// `crashup`), band -1 always fires slot 2, band 1 is the tires
    /// screech. Every trigger honors `mutes`.
    pub fn crash(&mut self, var1: f32, var2: i32) {
        if self.bfcrash == 0 {
            if var2 == 0 {
                if var1.abs() > 25.0 && var1.abs() < 170.0 {
                    if !self.mutes
                        && let Some(l) = self.lowcrash[self.crshturn as usize].as_mut()
                    {
                        l.play();
                    }
                    self.bfcrash = 2;
                }

                if var1.abs() > 170.0 {
                    if !self.mutes
                        && let Some(c) = self.crash[self.crshturn as usize].as_mut()
                    {
                        c.play();
                    }
                    self.bfcrash = 2;
                }

                if var1.abs() > 25.0 {
                    if self.crashup {
                        self.crshturn -= 1;
                    } else {
                        self.crshturn += 1;
                    }

                    if self.crshturn == -1 {
                        self.crshturn = 2;
                    }

                    if self.crshturn == 3 {
                        self.crshturn = 0;
                    }
                }
            }

            if var2 == -1 {
                if var1.abs() > 25.0 && var1.abs() < 170.0 {
                    if !self.mutes
                        && let Some(l) = self.lowcrash[2].as_mut()
                    {
                        l.play();
                    }
                    self.bfcrash = 2;
                }

                if var1.abs() > 170.0 {
                    if !self.mutes
                        && let Some(c) = self.crash[2].as_mut()
                    {
                        c.play();
                    }
                    self.bfcrash = 2;
                }
            }

            if var2 == 1 {
                if !self.mutes
                    && let Some(t) = self.tires.as_mut()
                {
                    t.play();
                }
                self.bfcrash = 3;
            }
        }
    }

    /// `xtGraphics.skid(int, float)` (xt:3414-3439): the skid/dust-skid
    /// alternates (`skflg`/`dskflg`), debounced 35 ticks by `bfskid`
    /// and gated on `bfcrash` + the 150 speed floor. Band 0 is the
    /// road skid, anything else the dust skid; both honor `mutes`.
    pub fn skid(&mut self, var1: i32, var2: f32) {
        if self.bfcrash == 0 && self.bfskid == 0 && var2 > 150.0 {
            if var1 == 0 {
                if !self.mutes
                    && let Some(s) = self.skid[self.skflg as usize].as_mut()
                {
                    s.play();
                }

                if self.skflg == 0 {
                    self.skflg = 1;
                } else {
                    self.skflg = 0;
                }
            } else {
                if !self.mutes
                    && let Some(s) = self.dustskid[self.dskflg as usize].as_mut()
                {
                    s.play();
                }

                if self.dskflg == 0 {
                    self.dskflg = 1;
                } else {
                    self.dskflg = 0;
                }
            }

            self.bfskid = 35;
        }
    }

    /// `xtGraphics.stopairs()` (xt:3901-3907): stop all six `air` loops.
    pub fn stopairs(&mut self) {
        for a in self.air.iter_mut().flatten() {
            a.stop();
        }
    }

    /// `xtGraphics.stopallnow()` (xt:3530-3556): the applet-lifecycle
    /// teardown — every armed track `outwithit()`, then the `cars` and
    /// `stages` voices. The `engs`/`air`/`wastd` `AudioClip` stops ride
    /// with the audio slice. Driven by the mirror's lifecycle/shutdown
    /// pins and the shell's stop path.
    pub fn stopallnow(&mut self) {
        for i in 0..11 {
            if self.loadedt[i]
                && let Some(r) = self.stracks[i].as_mut()
            {
                r.outwithit();
            }
        }
        for row in self.engs.iter_mut() {
            for e in row.iter_mut().flatten() {
                e.stop();
            }
        }
        for a in self.air.iter_mut().flatten() {
            a.stop();
        }
        if let Some(w) = self.wastd.as_mut() {
            w.stop();
        }
        if let Some(c) = self.cars.as_mut() {
            c.outwithit();
        }
        if let Some(s) = self.stages.as_mut() {
            s.outwithit();
        }
    }

    /// Bind every loaded voice to the mix bus: the 11 `stracks` (at their
    /// native mix rates), `stages`/`cars`, and every loaded `.au` clip
    /// (at its header rate). Each voice registers with its current play
    /// state and keeps its handle, so later `play`/`resume`/`stop` calls
    /// forward to the bus and `posit()` reads the device-side cursor.
    /// Idempotent per voice (bound voices are skipped), so late
    /// `loadmusic` builds join on the next pass instead of staying
    /// silent. Unloaded (`None`) slots and empty clips are skipped.
    pub fn bind_audio(&mut self, bus: &Arc<Mutex<AudioBus>>) {
        for track in self.stracks.iter_mut().flatten() {
            track.bind_audio(bus);
        }
        for voice in [&mut self.stages, &mut self.cars].into_iter().flatten() {
            voice.bind_audio(bus);
        }
        for voice in self.engs.iter_mut().flatten().flatten() {
            voice.bind_audio(bus);
        }
        for voice in self
            .air
            .iter_mut()
            .flatten()
            .chain(self.crash.iter_mut().flatten())
            .chain(self.lowcrash.iter_mut().flatten())
            .chain(self.skid.iter_mut().flatten())
            .chain(self.dustskid.iter_mut().flatten())
        {
            voice.bind_audio(bus);
        }
        for voice in [
            &mut self.tires,
            &mut self.checkpoint,
            &mut self.carfixed,
            &mut self.powerup,
            &mut self.three,
            &mut self.two,
            &mut self.one,
            &mut self.go,
            &mut self.wastd,
            &mut self.firewasted,
        ]
        .into_iter()
        .flatten()
        {
            voice.bind_audio(bus);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audioclip::get_sound;
    use crate::xtgraphics::testutil::armed_stage1;

    /// A bare engine rig: fresh voice, fase 0, one loaded `b2` voice on
    /// the car-0 row (`enginsignature[0][2] == 1`, i.e. `engs[1][2]`).
    fn engine_rig() -> (XtGraphics, Madness, Control, Medium) {
        let mut xt = XtGraphics::new();
        xt.fase = 0;
        xt.engs[1][2] = Some(get_sound("sounds/newsun/b2.au"));
        let mut mad = Madness::new(0);
        mad.mtouch = true;
        mad.speed = 25.0;
        let mut ctl = Control::new();
        ctl.up = true;
        let m = Medium::new();
        (xt, mad, ctl, m)
    }

    /// `playsounds`' mutem round-trip (xt:905-917): mute stops the
    /// armed track, unmute resumes it; the mutes flag syncs.
    #[test]
    fn playsounds_mutem_round_trip() {
        let mut xt = armed_stage1();
        xt.fase = 0;
        let mad = Madness::new(0);
        let mut ctl = Control::new();
        let mut med = Medium::new();

        ctl.mutem = true;
        xt.playsounds(&mad, &ctl, 1, &mut med);
        assert!(xt.mutem);
        assert!(!xt.stracks[0].as_ref().unwrap().playing);

        ctl.mutem = false;
        xt.playsounds(&mad, &ctl, 1, &mut med);
        assert!(!xt.mutem);
        assert!(xt.stracks[0].as_ref().unwrap().playing);

        // the mutes twin is a bare flag sync
        ctl.mutes = true;
        xt.playsounds(&mad, &ctl, 1, &mut med);
        assert!(xt.mutes);
        assert!(xt.stracks[0].as_ref().unwrap().playing);
    }

    /// `stopallnow` (xt:3530-3556) outwithits every armed track and both
    /// jingles; the arm flags are untouched.
    #[test]
    fn stopallnow_outwithits_everything_armed() {
        let mut xt = armed_stage1();
        let raw: &[u8] = include_bytes!("../../../../assets/music/stages.zipo");
        let z = Zipo::read(raw).unwrap();
        let mut s = RadicalMod::new(&z.entries[0].data, 200, 9000, 145);
        s.play();
        xt.stages = Some(s);
        xt.stopallnow();
        assert!(!xt.stracks[0].as_ref().unwrap().playing);
        assert!(!xt.stages.as_ref().unwrap().playing);
        assert!(xt.loadedt[0]);
    }

    /// The engine ladder (xt:757-822): speed 25 in car 0's first gear
    /// band keys voice 2 (`pengs[2]`, `engs[1][2]` looping); the other
    /// slots stay dark.
    #[test]
    fn playsounds_engine_ladder_loops_voice_2() {
        let (mut xt, mad, ctl, mut m) = engine_rig();
        xt.playsounds(&mad, &ctl, 1, &mut m);
        assert_eq!(xt.pengs, [false, false, true, false, false]);
        let e = xt.engs[1][2].as_ref().unwrap();
        assert!(e.playing && e.looping);
        assert_eq!(xt.pwait, 7);
    }

    /// The power-98 branch (xt:813-823): flat-out speed keys voice 3
    /// (`b3`), and the mute gate silences the whole hub (`sparkeng(-2)`).
    #[test]
    fn playsounds_power98_and_mute_gate() {
        let (mut xt, mut mad, ctl, mut m) = engine_rig();
        mad.power = 98.0;
        mad.speed = 100.0;
        xt.engs[1][3] = Some(get_sound("sounds/newsun/b3.au"));
        xt.playsounds(&mad, &ctl, 1, &mut m);
        assert_eq!(xt.pengs, [false, false, false, true, false]);
        assert!(xt.engs[1][3].as_ref().unwrap().looping);

        // the mute gate parks the hub: voices stop, arms drop
        xt.mutes = true;
        xt.playsounds(&mad, &ctl, 1, &mut m);
        assert_eq!(xt.pengs, [false; 5]);
        assert!(!xt.engs[1][3].as_ref().unwrap().playing);
    }

    /// The airborne branch (xt:843-857): untouched wheels loop an `air`
    /// voice and arm `aird` with the 10-tick stop.
    #[test]
    fn playsounds_airborne_loops_air() {
        let (mut xt, mut mad, ctl, mut m) = engine_rig();
        mad.mtouch = false;
        mad.wtouch = false;
        mad.speed = 0.0;
        for (i, name) in ["air0", "air1", "air2", "air3"].iter().enumerate() {
            xt.air[i] = Some(get_sound(&format!("sounds/newsun/{name}.au")));
        }
        xt.playsounds(&mad, &ctl, 1, &mut m);
        assert_eq!(xt.pwait, 15);
        assert!(xt.aird);
        // the arm sets 10, then the same call's decay ticks it to 9
        assert_eq!(xt.stopcnt, 9);
        assert!(
            xt.air.iter().flatten().any(|a| a.playing),
            "an air voice loops"
        );
    }

    /// The wasted loop (xt:860-874): `cntdest` arms `wastd.loop()`; the
    /// cntwis-7 tail fires `firewasted` once.
    #[test]
    fn playsounds_wasted_loop_and_fire() {
        let (mut xt, mut mad, ctl, mut m) = engine_rig();
        mad.mtouch = false;
        mad.wtouch = true;
        mad.cntdest = 5;
        xt.wastd = Some(get_sound("sounds/newsun/wasted.au"));
        xt.firewasted = Some(get_sound("sounds/newsun/firewasted.au"));
        xt.playsounds(&mad, &ctl, 1, &mut m);
        assert!(xt.pwastd);
        assert!(xt.wastd.as_ref().unwrap().looping);

        mad.cntdest = 5;
        xt.cntwis = 7;
        xt.playsounds(&mad, &ctl, 1, &mut m);
        assert!(!xt.pwastd);
        assert!(!xt.wastd.as_ref().unwrap().playing);
        assert!(xt.firewasted.as_ref().unwrap().playing);
        assert!(!xt.firewasted.as_ref().unwrap().looping);
    }

    /// The per-tick decay (xt:883-897) runs outside the race gate.
    #[test]
    fn playsounds_decay_counters() {
        let (mut xt, mad, ctl, mut m) = engine_rig();
        xt.fase = 6;
        xt.stopcnt = 5;
        xt.bfcrash = 2;
        xt.bfskid = 3;
        xt.playsounds(&mad, &ctl, 1, &mut m);
        assert_eq!(xt.stopcnt, 4);
        assert_eq!(xt.bfcrash, 1);
        assert_eq!(xt.bfskid, 2);

        // stopcnt == 1 also sweeps the air loops on the way down
        xt.air[1] = Some(get_sound("sounds/newsun/air1.au"));
        xt.air[1].as_mut().unwrap().loop_();
        xt.stopcnt = 1;
        xt.playsounds(&mad, &ctl, 1, &mut m);
        assert_eq!(xt.stopcnt, 0);
        assert!(!xt.air[1].as_ref().unwrap().playing, "stopairs swept");
    }

    /// `crash` (xt:935-1006): the low band round-robins, the mid band
    /// hits hard, band -1 always fires slot 2, band 1 is the tires
    /// screech; `bfcrash` debounces even when muted.
    #[test]
    fn crash_picker_bands_and_debounce() {
        let mut xt = XtGraphics::new();
        xt.lowcrash[0] = Some(get_sound("sounds/newsun/lowcrash1.au"));
        xt.crash[0] = Some(get_sound("sounds/newsun/crash1.au"));
        xt.crash[2] = Some(get_sound("sounds/newsun/crash3.au"));
        xt.tires = Some(get_sound("sounds/newsun/tires.au"));

        // low band: slot 0 fires, debounce arms, turn advances
        xt.crash(100.0, 0);
        assert!(xt.lowcrash[0].as_ref().unwrap().playing);
        assert_eq!(xt.bfcrash, 2);
        assert_eq!(xt.crshturn, 1);

        // debounced: nothing more fires
        xt.crash(200.0, 0);
        assert!(!xt.crash[0].as_ref().is_some_and(|c| c.playing));

        // muted mid band: silent, but the debounce still arms
        xt.bfcrash = 0;
        xt.crshturn = 0;
        xt.mutes = true;
        xt.crash(200.0, 0);
        assert_eq!(xt.bfcrash, 2);
        assert!(!xt.crash[0].as_ref().unwrap().playing);
        assert_eq!(xt.crshturn, 1, "the turn advances even muted");

        // band -1 always fires slot 2
        xt.bfcrash = 0;
        xt.mutes = false;
        xt.crash(200.0, -1);
        assert!(xt.crash[2].as_ref().unwrap().playing);

        // band 1 is the tires screech with a 3-tick debounce
        xt.bfcrash = 0;
        xt.crash(50.0, 1);
        assert!(xt.tires.as_ref().unwrap().playing);
        assert_eq!(xt.bfcrash, 3);
    }

    /// Crash-grind stacking (GameSparker.java:860-936): band -1 always
    /// fires slot 2, and a grind re-fires every ~2 frames while the
    /// ~1.5 s clip still sounds — the second trigger stacks an overlap
    /// instead of truncating the first (the JDK spawns a player per
    /// `play()`), so both copies stay armed on the bus.
    #[test]
    fn crash_retrigger_stacks_overlapping_voices() {
        use std::sync::{Arc, Mutex};

        let bus = Arc::new(Mutex::new(crate::audiobus::AudioBus::new()));
        let mut xt = XtGraphics::new();
        xt.crash[2] = Some(get_sound("sounds/newsun/crash3.au"));
        xt.bind_audio(&bus);
        xt.bfcrash = 0;
        xt.crash(200.0, -1);
        assert!(xt.crash[2].as_ref().unwrap().playing);
        xt.bfcrash = 0;
        xt.crash(200.0, -1);
        let b = bus.lock().unwrap();
        assert_eq!(b.voice_count(), 2, "primary + one overlap");
        assert!(b.is_playing(0) && b.is_playing(1));
    }

    /// Powerup wash stacking (GameSparker.java:xt:1982-1990 — the stat
    /// stunt block re-plays `powerup` every frame while `auscnt < 45`,
    /// ~15+ consecutive triggers of a ~1.5 s sting): the second `play()`
    /// while the first still sounds stacks instead of restarting, so the
    /// chorus the JDK mixes survives instead of buzzing on frame one.
    #[test]
    fn powerup_restack_stacks_overlapping_voices() {
        use std::sync::{Arc, Mutex};

        let bus = Arc::new(Mutex::new(crate::audiobus::AudioBus::new()));
        let mut xt = XtGraphics::new();
        xt.powerup = Some(get_sound("sounds/newsun/powerup.au"));
        xt.bind_audio(&bus);
        xt.powerup.as_mut().unwrap().play();
        xt.powerup.as_mut().unwrap().play();
        let b = bus.lock().unwrap();
        assert_eq!(b.voice_count(), 2, "primary + one overlap");
        assert!(b.is_playing(0) && b.is_playing(1));
    }

    /// `skid` (xt:3414-3439): the road/dust alternates flip, debounced
    /// 35 ticks by `bfskid`, gated on speed and `bfcrash`.
    #[test]
    fn skid_alternates_and_debounces() {
        let mut xt = XtGraphics::new();
        xt.skid[0] = Some(get_sound("sounds/newsun/skid1.au"));
        xt.dustskid[0] = Some(get_sound("sounds/newsun/dustskid1.au"));

        xt.skid(0, 200.0);
        assert!(xt.skid[0].as_ref().unwrap().playing);
        assert_eq!(xt.skflg, 1);
        assert_eq!(xt.bfskid, 35);

        // debounced: the flip holds
        xt.skid(0, 200.0);
        assert_eq!(xt.skflg, 1);

        // muted dust band: silent, but flips and debounces
        xt.bfskid = 0;
        xt.mutes = true;
        xt.skid(2, 200.0);
        assert!(!xt.dustskid[0].as_ref().unwrap().playing);
        assert_eq!(xt.dskflg, 1);
        assert_eq!(xt.bfskid, 35);

        // slow or crash-blocked: inert
        xt.bfskid = 0;
        xt.bfcrash = 1;
        xt.mutes = false;
        xt.skid(0, 200.0);
        assert_eq!(xt.skflg, 1, "bfcrash blocks");
        xt.bfcrash = 0;
        xt.skid(0, 100.0);
        assert_eq!(xt.skflg, 1, "the 150 floor holds");
    }

    /// `stopallnow` also silences the armed `AudioClip` voices.
    #[test]
    fn stopallnow_stops_clip_voices() {
        let mut xt = XtGraphics::new();
        xt.loadedt[0] = true;
        xt.engs[0][0] = Some(get_sound("sounds/newsun/a0.au"));
        xt.engs[0][0].as_mut().unwrap().loop_();
        xt.pengs[0] = true;
        xt.air[2] = Some(get_sound("sounds/newsun/air2.au"));
        xt.air[2].as_mut().unwrap().loop_();
        xt.wastd = Some(get_sound("sounds/newsun/wasted.au"));
        xt.wastd.as_mut().unwrap().loop_();
        xt.stopallnow();
        assert!(!xt.engs[0][0].as_ref().unwrap().playing);
        assert!(!xt.air[2].as_ref().unwrap().playing);
        assert!(!xt.wastd.as_ref().unwrap().playing);
    }

    // `stageselect`'s masked stage-1 capture is folded into the full-frame
    // row `screen_stageselect_stage1` (and stage 5/11 + pressed-button
    // variants), strictly stronger: the rows include the title, name and
    // hint text the mask dropped. The key routing test below stays.
}
