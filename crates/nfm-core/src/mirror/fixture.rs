//! The `tests/fixtures/toml/*.toml` loader: the real `toml` crate (a
//! dev/test-support dependency only — `nfm-core` itself stays
//! zero-new-dependency) behind the `Toml.java` accessor API, so the Java
//! and Rust consumers navigate the fixtures identically
//! (tests/fixtures/toml/README.md is the contract).
//!
//! Loud failures with file and key path, never silent defaults. The Java
//! reader adds the defining LINE to each failure; the crate's dynamic
//! `Value` does not carry spans, so the Rust messages carry file + key
//! path only — a documented harness difference (PHASE-B ledger). Loads
//! are not cached, like Toml.java. Values must stay inside the documented
//! subset; the fixtures are frozen and in-subset.

use std::path::PathBuf;

pub struct Tml {
    file: String,
    /// Dotted key path of this view ("" = root), for error text.
    path: String,
    v: toml::Value,
}

impl Tml {
    pub fn load(rel: &str) -> Tml {
        let p = root().join(rel);
        let text = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("toml: cannot read {}: {e}", p.display()));
        let v: toml::Value =
            toml::from_str(&text).unwrap_or_else(|e| panic!("toml {rel}: parse error: {e}"));
        Tml {
            file: rel.to_string(),
            path: String::new(),
            v,
        }
    }

    /// True when the key is present in this table (`Toml.has`).
    pub fn has(&self, key: &str) -> bool {
        self.v.get(key).is_some()
    }

    /// The keys of this table, sorted (stable for shape/count pins;
    /// `Toml.keys` uses a TreeMap — the Rust map is sorted here to match).
    pub fn keys(&self) -> Vec<String> {
        match self.v.as_table() {
            Some(t) => {
                let mut out: Vec<String> = t.keys().cloned().collect();
                out.sort();
                out
            }
            None => self.fail_current("table"),
        }
    }

    /// A subtable; fails unless present and a `[table]` (`Toml.table`).
    pub fn table(&self, key: &str) -> Tml {
        let v = self.get(key);
        match v.as_table() {
            Some(_) => Tml {
                file: self.file.clone(),
                path: self.join(key),
                v: v.clone(),
            },
            None => self.fail(key, "table"),
        }
    }

    /// The rows of an `[[array.of.tables]]` (`Toml.tables`).
    pub fn tables(&self, key: &str) -> Vec<Tml> {
        let v = self.get(key);
        let arr = match v.as_array() {
            Some(a) => a,
            None => self.fail(key, "array of tables"),
        };
        let mut out = Vec::new();
        for (i, e) in arr.iter().enumerate() {
            if e.as_table().is_none() {
                self.fail_at(key, i, "table row");
            }
            out.push(Tml {
                file: self.file.clone(),
                path: format!("{}{}[{}]", self.path, key, out.len()),
                v: e.clone(),
            });
        }
        out
    }

    /// `Toml.tableCount`.
    pub fn table_count(&self, key: &str) -> usize {
        self.tables(key).len()
    }

    /// `Toml.tableAt`.
    pub fn table_at(&self, key: &str, i: usize) -> Tml {
        let ts = self.tables(key);
        match ts.get(i) {
            Some(t) => Tml {
                file: t.file.clone(),
                path: t.path.clone(),
                v: t.v.clone(),
            },
            None => panic!(
                "toml {}: {}{}[{}]: missing row",
                self.file, self.path, key, i
            ),
        }
    }

    /// `Toml.str`.
    pub fn str(&self, key: &str) -> String {
        let v = self.get(key);
        match v.as_str() {
            Some(s) => s.to_string(),
            None => self.fail(key, "string"),
        }
    }

    /// `Toml.intv` — an integer range-checked into i32 (the game's
    /// integer width).
    pub fn intv(&self, key: &str) -> i32 {
        let l = self.longv(key);
        if l < i32::MIN as i64 || l > i32::MAX as i64 {
            self.fail(key, "int-range integer");
        }
        l as i32
    }

    /// `Toml.longv`.
    pub fn longv(&self, key: &str) -> i64 {
        let v = self.get(key);
        match v.as_integer() {
            Some(l) => l,
            None => self.fail(key, "integer"),
        }
    }

    /// `Toml.floatv` — a number as f32; an integer-valued TOML number is
    /// accepted.
    pub fn floatv(&self, key: &str) -> f32 {
        let v = self.get(key);
        match v.as_float() {
            Some(f) => f as f32,
            None => match v.as_integer() {
                Some(l) => l as f32,
                None => self.fail(key, "number"),
            },
        }
    }

    /// `Toml.boolv`.
    pub fn boolv(&self, key: &str) -> bool {
        let v = self.get(key);
        match v.as_bool() {
            Some(b) => b,
            None => self.fail(key, "boolean"),
        }
    }

    /// `Toml.intArr`.
    pub fn int_arr(&self, key: &str) -> Vec<i32> {
        self.long_arr(key)
            .into_iter()
            .map(|l| {
                if l < i32::MIN as i64 || l > i32::MAX as i64 {
                    self.fail(key, "int-range integer element");
                }
                l as i32
            })
            .collect()
    }

    /// `Toml.longArr`.
    pub fn long_arr(&self, key: &str) -> Vec<i64> {
        let v = self.get(key);
        let arr = match v.as_array() {
            Some(a) => a,
            None => self.fail(key, "array"),
        };
        let mut out = Vec::new();
        for (i, e) in arr.iter().enumerate() {
            match e.as_integer() {
                Some(l) => out.push(l),
                None => self.fail_at(key, i, "integer element"),
            }
        }
        out
    }

    /// `Toml.floatArr` — integer-valued elements are accepted.
    pub fn float_arr(&self, key: &str) -> Vec<f32> {
        let v = self.get(key);
        let arr = match v.as_array() {
            Some(a) => a,
            None => self.fail(key, "array"),
        };
        let mut out = Vec::new();
        for (i, e) in arr.iter().enumerate() {
            match (e.as_float(), e.as_integer()) {
                (Some(f), _) => out.push(f as f32),
                (None, Some(l)) => out.push(l as f32),
                (None, None) => self.fail_at(key, i, "number element"),
            }
        }
        out
    }

    /// `Toml.strArr`.
    #[allow(dead_code)] // Toml.java API surface; no suite pin calls this accessor — kept for reader-shape parity
    pub fn str_arr(&self, key: &str) -> Vec<String> {
        let v = self.get(key);
        let arr = match v.as_array() {
            Some(a) => a,
            None => self.fail(key, "array"),
        };
        let mut out = Vec::new();
        for (i, e) in arr.iter().enumerate() {
            match e.as_str() {
                Some(s) => out.push(s.to_string()),
                None => self.fail_at(key, i, "string element"),
            }
        }
        out
    }

    /// Fails unless every named key is present (`Toml.requireKeys`).
    pub fn require_keys(&self, keys: &[&str]) {
        for k in keys {
            if !self.has(k) {
                panic!(
                    "toml {}: {}{}: missing required key",
                    self.file,
                    self.path,
                    self.join_key(k)
                );
            }
        }
    }

    #[allow(dead_code)] // Toml.java API surface; no suite pin calls this accessor — kept for reader-shape parity
    pub fn file(&self) -> &str {
        &self.file
    }

    // ── internals ───────────────────────────────────────────────────────

    fn get(&self, key: &str) -> toml::Value {
        match self.v.get(key) {
            Some(v) => v.clone(),
            None => panic!(
                "toml {}: {}{}: missing key",
                self.file,
                self.path,
                self.join_key(key)
            ),
        }
    }

    fn join(&self, key: &str) -> String {
        format!("{}{}", self.path, key)
    }

    fn join_key(&self, key: &str) -> String {
        if self.path.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", self.path, key)
        }
    }

    fn fail(&self, key: &str, want: &str) -> ! {
        panic!(
            "toml {}: {}: expected {}, got {}",
            self.file,
            self.join(key),
            want,
            kind(
                &self
                    .v
                    .get(key)
                    .cloned()
                    .unwrap_or(toml::Value::Boolean(false))
            )
        )
    }

    fn fail_current(&self, want: &str) -> ! {
        panic!("toml {}: {}: expected {}", self.file, self.path, want)
    }

    fn fail_at(&self, key: &str, i: usize, want: &str) -> ! {
        panic!(
            "toml {}: {}{}[{}]: expected {}",
            self.file, self.path, key, i, want
        )
    }
}

fn kind(v: &toml::Value) -> &'static str {
    match v {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}

/// The fixture root: cargo runs unit tests with the package directory as
/// the working directory, and the suite may also be driven from the repo
/// root — probe both, like the port's asset-root resolution.
fn root() -> PathBuf {
    for cand in ["../../tests/fixtures/toml", "tests/fixtures/toml"] {
        let p = PathBuf::from(cand);
        if p.is_dir() {
            return p;
        }
    }
    panic!("toml fixtures not found (run cargo from the repo root or crates/nfm-core)")
}

/// Load-shape smoke over all five fixture files: harness-level shape
/// assertions (README consumption rule 1). Ids carry the `smoke_load_`
/// prefix — they are HARNESS assertions, not Java pin twins; the Java
/// shape pins live inside the T-classes and their twins arrive with each
/// class batch.
pub(crate) fn smoke() {
    crate::mirror::harness::group("Fixtures");
    use crate::mirror::harness as check;

    // determinism.toml — seeds, vectors, recipes, walks
    let det = Tml::load("determinism.toml");
    det.require_keys(&["seed", "vectors", "recipes", "walks"]);
    det.table("seed").require_keys(&["pixel_main"]);
    let vec = det.table("vectors");
    vec.require_keys(&["vecmedium", "record_tenths", "contodraw_tenths"]);
    check::eq_i32(
        vec.float_arr("vecmedium").len() as i32,
        12,
        "smoke_load_vecmedium_len",
    );
    check::eq_i32(
        vec.float_arr("record_tenths").len() as i32,
        12,
        "smoke_load_record_tenths_len",
    );
    check::eq_i32(
        vec.float_arr("contodraw_tenths").len() as i32,
        48,
        "smoke_load_contodraw_tenths_len",
    );
    let cp = det.table("recipes").table("control_preset");
    cp.require_keys(&[
        "cntrn",
        "rand",
        "diup",
        "trn",
        "walk",
        "walk_cntrn",
        "walk_trn",
        "walk_rand",
        "alt_rand",
        "alt_first",
    ]);
    check::eq_i32(
        cp.float_arr("walk").len() as i32,
        12,
        "smoke_load_control_walk_len",
    );
    check::eq_i32(
        cp.int_arr("rand").len() as i32,
        3,
        "smoke_load_control_rand_len",
    );
    let walks = det.table("walks");
    walks.require_keys(&["crown", "crown_passes"]);
    check::eq_i32(
        walks.int_arr("crown").len() as i32,
        10,
        "smoke_load_crown_len",
    );
    check::eq_i32(walks.intv("crown_passes"), 295, "smoke_load_crown_passes");

    // audio.toml — 11 stage rows, each with the full 16-key shape
    let audio = Tml::load("audio.toml");
    check::eq_i32(
        audio.table_count("stage") as i32,
        11,
        "smoke_load_audio_stages",
    );
    for s in audio.tables("stage").iter() {
        s.require_keys(&[
            "id",
            "path",
            "gain",
            "rate",
            "bpm",
            "decode_bytes",
            "decode_md5",
            "mix_calls",
            "mix_samples",
            "mix_md5",
            "num_tracks",
            "num_patterns",
            "song_length_patterns",
            "song_repeat_patterns",
            "instruments",
            "bpm_samples",
        ]);
    }

    // pixel.toml — atlas geometry, probes, the frames reference, frame size
    let pixel = Tml::load("pixel.toml");
    pixel.require_keys(&["atlas", "probes", "frames", "frame"]);
    let atlas = pixel.table("atlas");
    atlas.require_keys(&[
        "columns",
        "rows",
        "cell_w",
        "cell_h",
        "glyph_dx",
        "baseline",
        "codepoint_min",
        "codepoint_max",
        "sizes",
        "format",
        "ttf_path",
        "sha256_ttf",
        "sha256_png",
        "sha256_manifest",
    ]);
    check::eq_i32(atlas.intv("columns"), 95, "smoke_load_atlas_columns");
    check::eq_i32(atlas.intv("rows"), 2, "smoke_load_atlas_rows");
    check::eq_i32(
        atlas.int_arr("sizes").len() as i32,
        2,
        "smoke_load_atlas_sizes_len",
    );
    pixel
        .table("probes")
        .require_keys(&["font_probe_13_md5", "font_probe_11_md5"]);
    let frames = pixel.table("frames");
    frames.require_keys(&["file", "row_count"]);
    // 84 = the 83 rows + `screen_carselect_car4_stats` (the play-truth
    // stretch's sanctioned freeze-protocol change; pixel.toml [frames]).
    check::eq_i32(frames.intv("row_count"), 84, "smoke_load_frame_rows_84");
    let frame = pixel.table("frame");
    check::eq_i32(frame.intv("w"), 550, "smoke_load_frame_w");
    check::eq_i32(frame.intv("h"), 400, "smoke_load_frame_h");

    // constants.toml — the 11 load-bearing sections
    let constants = Tml::load("constants.toml");
    check::eq_i32(
        constants.keys().len() as i32,
        11,
        "smoke_load_constants_sections",
    );
    constants.require_keys(&[
        "madness",
        "control",
        "xt",
        "conto",
        "plane",
        "wheels",
        "record",
        "gamesparker",
        "modslayer",
        "audio_device",
        "medium",
    ]);

    // scenarios.toml — 4 scenario rows; the [[scenario.phase]] nesting
    // resolves through the intermediate-binding rule.
    let scen = Tml::load("scenarios.toml");
    let count = scen.table_count("scenario");
    check::eq_i32(count as i32, 4, "smoke_load_scenarios");
    for s in scen.tables("scenario").iter() {
        s.require_keys(&["id", "ticks"]);
    }
}
