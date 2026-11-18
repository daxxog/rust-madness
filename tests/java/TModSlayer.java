import java.util.zip.CRC32;

/**
 * Pins ModSlayer: the beattrack effect handlers (the six live bits),
 * startplaying's formulas, updatetracks' pattern-advance state machine,
 * and the E2E turnbytes() output for stage1.zipo at the game's exact
 * parameters - whose md5 equals the Rust port's stored digest
 * f1f2a25eb468517faa2218cf1712395f, proving the port's mixer
 * byte-faithful against the original JDK.
 * Pin source: notes/JAVA-BEHAVIOR/B2_mod_modslayer.md (verified).
 */
public class TModSlayer {
   static void run() throws Exception {
      Check.group("ModSlayer");
      TestApplet app = new TestApplet("assets/music");

      // ---- the beattrack effect handlers ----
      // guard: period_low_limit==0 becomes 1 (division protection)
      ModTrackInfo t = new ModTrackInfo();
      t.period_low_limit = 0;
      t.period_high_limit = 99999;
      Mod m4 = fixture(app, 4, false);
      ModSlayer sl = new ModSlayer(m4, 8000, 350, 125);
      t.finetune_rate = 8000;
      sl.beattrack(t);
      Check.eq(t.period_low_limit, 1, "guard_low_limit");

      // VOL_SLIDE (effect&1): volume clamped to [0, 64]
      t = mti(1, 10, 0);
      t.vol_slide = 5;
      sl.beattrack(t);
      Check.eq(t.volume, 15, "vol_slide_add");
      t = mti(1, 70, 0);
      sl.beattrack(t);
      Check.eq(t.volume, 64, "vol_slide_high_clamp");
      t = mti(1, -5, 0);
      sl.beattrack(t);
      Check.eq(t.volume, 0, "vol_slide_low_clamp");

      // PORT_DOWN (effect&2): period += port_down, clamp to high_limit,
      // then pitch = finetune_rate / period
      t = mti(2, 0, 100);
      t.port_down = 50;
      t.period_high_limit = 1712;
      sl.beattrack(t);
      Check.eq(t.period, 150, "port_down_add");
      Check.eq(t.pitch, 8000 / 150, "port_down_pitch");
      t = mti(2, 0, 1700);
      t.port_down = 50;
      t.period_high_limit = 1712;
      sl.beattrack(t);
      Check.eq(t.period, 1712, "port_down_clamp");

      // PORT_UP (effect&4): the S3M underflow quirk - clamps to the
      // period_HIGH_limit on s3m, the low limit otherwise
      Mod ms3m = fixture(app, 4, true);
      ModSlayer sls3m = new ModSlayer(ms3m, 8000, 350, 125);
      t = mti(4, 0, 50);
      t.port_up = 100;
      t.period_low_limit = 10;
      t.period_high_limit = 200;
      sls3m.beattrack(t);
      Check.eq(t.period, 200, "port_up_s3m_underflow_goes_HIGH");
      t = mti(4, 0, 50);
      t.port_up = 100;
      t.period_low_limit = 10;
      t.period_high_limit = 200;
      sl.beattrack(t);
      Check.eq(t.period, 10, "port_up_mod_underflow_goes_low");

      // PORT_TO (effect&32): approaches portto from both sides
      t = mti(32, 0, 100);
      t.portto = 500;
      t.port_inc = 700;
      sl.beattrack(t);
      Check.eq(t.period, 500, "port_to_up_lands");
      t = mti(32, 0, 500);
      t.portto = 100;
      t.port_inc = 700;
      sl.beattrack(t);
      Check.eq(t.period, 100, "port_to_down_lands");

      // VIBRATO (effect&8): vibpos += rate<<2; second sine half (bit
      // 128) NEGATES the offset; pitch = finetune_rate / (period+off)
      t = mti(8, 0, 0);
      t.vib_rate = 1;
      t.vib_depth = 128;
      t.period_low_limit = 1;
      t.period_high_limit = 99999;
      sl.beattrack(t);
      Check.eq(t.vibpos, 4, "vibrato_phase_step");
      Check.eq(t.pitch, 8000 / (ModSlayer.sintable[1]), "vibrato_first_half");

      t = mti(8, 0, 1000);
      t.vibpos = 124;
      t.vib_rate = 2;
      t.vib_depth = 128;
      t.period_low_limit = 1;
      t.period_high_limit = 99999;
      sl.beattrack(t);
      // vibpos 124+8=132: index 132>>2&31 = 1 (same entry), but bit 128
      // set -> the offset NEGATES: period 1000 - sintable[1]
      Check.eq(t.vibpos, 132, "vibrato_phase_b");
      Check.eq(t.pitch, 8000 / (1000 - ModSlayer.sintable[1]), "vibrato_second_half_negates");

      // ARPEGGIO (effect&16): pitch cycles through arp[0..2], wrap at 3
      t = mti(16, 0, 0);
      t.finetune_rate = 8000;
      t.arp[0] = 100;
      t.arp[1] = 200;
      t.arp[2] = 400;
      sl.beattrack(t);
      Check.eq(t.pitch, 80, "arpeggio_0");
      Check.eq(t.arpindex, 1, "arpeggio_index_1");
      sl.beattrack(t);
      Check.eq(t.pitch, 40, "arpeggio_1");
      sl.beattrack(t);
      Check.eq(t.pitch, 20, "arpeggio_2");
      Check.eq(t.arpindex, 0, "arpeggio_wrap");

      // ---- startplaying formulas ----
      // MOD finetune: 22748294283264 / (mixspeed * (1536 - value))
      Mod m1 = fixture(app, 4, false);
      m1.insts[0].finetune_value = 0;
      ModSlayer s1 = new ModSlayer(m1, 8000, 350, 125);
      s1.startplaying(false);
      Check.eq(m1.insts[0].finetune_rate,
         (int)(22748294283264L / (8000 * (1536 - 0))), "mod_finetune_formula");
      Check.eq(m1.insts[0].period_low_limit, 113, "mod_period_low");
      Check.eq(m1.insts[0].period_high_limit, 856, "mod_period_high");
      Check.eq(s1.bpm_samples, 8000 / (24 * 125 / 60), "bpm_samples_int_math");

      // S3M finetune: (428 * value << 8) / mixspeed
      Mod m1s = fixture(app, 4, true);
      m1s.insts[0].finetune_value = 1;
      ModSlayer s1s = new ModSlayer(m1s, 8000, 350, 125);
      s1s.startplaying(false);
      Check.eq(m1s.insts[0].finetune_rate,
         (int)(428L * 1 << 8) / 8000, "s3m_finetune_formula");
      Check.eq(m1s.insts[0].period_low_limit, 14, "s3m_period_low");
      Check.eq(m1s.insts[0].period_high_limit, 1712, "s3m_period_high");

      // vol_shift by numtracks: >8 -> 2, >4 -> 1, else 0
      Mod m6 = fixture(app, 6, false);
      ModSlayer s6 = new ModSlayer(m6, 8000, 350, 125);
      s6.startplaying(false);
      Check.eq(s6.vol_shift, 1, "vol_shift_6tracks");
      Mod m9 = fixture(app, 9, false);
      ModSlayer s9 = new ModSlayer(m9, 8000, 350, 125);
      s9.startplaying(false);
      Check.eq(s9.vol_shift, 2, "vol_shift_9tracks");
      Check.eq(s1.vol_shift, 0, "vol_shift_4tracks");

      // make_vol_table8 runs only when !bit16 (vol_table is byte[16640])
      s1.startplaying(false);
      Check.eq(s1.vol_table.length, 16640, "vol_table8_built");
      Mod m1b = fixture(app, 4, false);
      ModSlayer s1b = new ModSlayer(m1b, 8000, 350, 125);
      s1b.bit16 = true;
      s1b.startplaying(false);
      Check.ok(s1b.vol_table == null, "bit16_skips_vol_table8");

      // ---- updatetracks: the pattern-advance state machine ----
      Mod madv = fixture(app, 4, false);
      ModSlayer sadv = new ModSlayer(madv, 8000, 350, 125);
      sadv.startplaying(false);
      // startplaying leaves row=64: the first call wraps to row 1,
      // order_pos 0 -> 1, tempo_wait reset to the tempo
      sadv.updatetracks();
      Check.eq(sadv.row, 1, "updatetracks_row_wraps");
      Check.eq(sadv.order_pos, 1, "updatetracks_order_advances");
      Check.eq(sadv.tempo_wait, 6, "updatetracks_tempo_wait");
      // pattofs after one row = 4 bytes per note x numtracks - the
      // get_track stride
      Check.eq(sadv.pattofs, 4 * 4, "updatetracks_pattofs_row_stride");
      // 64 more calls: row walks 2..64, then the 65th call advances the
      // order again (positions[1] = 0 in the fixture)
      for (int i = 0; i < 63; i++) {
         sadv.updatetracks();
      }
      Check.eq(sadv.row, 64, "updatetracks_row_walks");
      Check.eq(sadv.order_pos, 1, "updatetracks_stays_until_wrap");
      sadv.updatetracks();
      Check.eq(sadv.order_pos, 2, "updatetracks_order_wraps_every_64");

      // mod_done: song_length 0 forces the wrap; nloops 1 -> 0 ends it
      Mod mdone = fixture(app, 4, false);
      mdone.song_length_patterns = 0;
      ModSlayer sdone = new ModSlayer(mdone, 8000, 350, 125);
      sdone.startplaying(false);
      Check.ok(!sdone.mod_done, "mod_done_not_yet");
      sdone.updatetracks();
      Check.ok(sdone.mod_done, "mod_done_after_wrap");
      Check.eq(sdone.nloops, 0, "mod_done_nloops");

      // ---- E2E: the game's real stage1 render ----
      // xtGraphics.java:1250 -> RadicalMod("music/stage1.zipo", 350,
      // 8400, 135) -> ModSlayer(mod, 8400, 350, 135); the md5 EQUALS the
      // Rust port's stored digest - the mixer is byte-faithful.
      Mod stage1 = new Mod("stage1.zipo", app);
      ModSlayer se2e = new ModSlayer(stage1, 8400, 350, 135);
      byte[] out = se2e.turnbytes();
      Check.eq(out.length, 1191330, "stage1_len");
      CRC32 crc = new CRC32();
      crc.update(out);
      Check.eq(crc.getValue(), 4025273850L, "stage1_crc32");
      java.security.MessageDigest md5alg = java.security.MessageDigest.getInstance("MD5");
      md5alg.update(out);
      StringBuilder hex = new StringBuilder();
      for (byte b : md5alg.digest()) {
         hex.append(String.format("%02x", b));
      }
      Check.eq(hex.toString(), "f1f2a25eb468517faa2218cf1712395f", "stage1_md5");
   }

   /** A ModTrackInfo with one effect bit, a volume and a period. */
   static ModTrackInfo mti(int effect, int volume, int period) {
      ModTrackInfo t = new ModTrackInfo();
      t.effect = effect;
      t.volume = volume;
      t.period = period;
      t.finetune_rate = 8000;
      t.period_low_limit = 1;
      t.period_high_limit = 99999;
      return t;
   }

   /** A hand-built broken-ctor Mod with the fields a mixer needs. */
   static Mod fixture(TestApplet app, int numtracks, boolean s3m) throws Exception {
      Mod m = new Mod("fixture.zipo", app);
      m.s3m = s3m;
      m.numtracks = numtracks;
      m.insts = new ModInstrument[]{new ModInstrument()};
      m.insts[0].finetune_value = 0;
      m.positions = new byte[128];
      m.positions[0] = 0;
      m.positions[1] = 0;
      m.song_length_patterns = 2;
      m.song_repeat_patterns = 2;
      m.patterns = new byte[256][numtracks * 4 * 64];
      m.name = "fixture";
      return m;
   }
}
