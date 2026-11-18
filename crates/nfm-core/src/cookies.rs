//! `cookies` — the persistence store standing in for the browser cookie jar.
//!
//! The source bridges to JavaScript (`GameSparker.java:150-162`):
//! `savecookie(key, value)` is `JSObject.getWindow(this).eval(`
//! `"SetCookie('key','value');")` and `readcookie(key)` evals
//! `s=GetCookie('key');` then `Integer.valueOf(getMember("s"))`, both with
//! swallowed exceptions and a −1 default.
//!
//! Documented divergence: the browser jar is per-origin and managed by the
//! page; the port keeps a local `key=value` text file (default
//! `nfm-cookies.txt` in the cwd, injectable for tests). Reads parse the
//! value as `i32` with −1 on missing/garbage — the source's
//! `Integer.valueOf` failure path. Writes upsert one `key=value` line
//! via a tmp-file rename (atomic enough headlessly).
//!
//! Path resolution (batch D2a) keeps the cwd **primary** — the dev flow and
//! the original behavior — and adds the directory of the running executable
//! as the **fallback**, so a Deck binary launched with an arbitrary cwd still
//! finds the jar written earlier beside its own binary. Reads try cwd then
//! the executable directory; writes try cwd and fall back on I/O failure.
//! An absolute path (how tests inject a scratch file) resolves to itself,
//! with no fallback.
//!
//! Divergence completed for the web port: the desktop build keeps the local
//! `key=value` file as its documented stand-in, while the wasm build keeps the
//! same jar text in **one `localStorage` entry (`nfm-cookies`)** — the
//! per-origin, page-managed storage the original applet actually used through
//! `JSObject` (T1's seam). The read/save entry points below branch on target:
//! the host path-resolution code is unchanged and simply not compiled on
//! wasm. `js_ls_get`/`js_ls_set` are supplied by `public/site/glue.js`; a
//! throwing `localStorage` (private mode) degrades to session-only there and
//! never traps here.

use std::path::{Path, PathBuf};

// The web host's jar bridge. Mirrors nfm-web's own `env` imports — undefined
// `extern "C"` symbols land in the `env` import module, which glue.js supplies.
// `js_ls_get` copies at most `cap` jar bytes into `buf` and returns the byte
// count; `js_ls_set` stores exactly `len` bytes at `ptr`. The JS side owns all
// error handling, so neither import can throw into wasm.
#[cfg(target_arch = "wasm32")]
unsafe extern "C" {
    fn js_ls_get(buf: *mut u8, cap: usize) -> usize;
    fn js_ls_set(ptr: *const u8, len: usize);
}

/// Bounded scratch for the web jar. A few dozen `key=value` lines fit easily;
/// 16 KiB keeps a hostile/oversized stored value from growing wasm memory
/// without bound.
#[cfg(target_arch = "wasm32")]
const LS_SCRATCH_CAP: usize = 16 * 1024;

/// The web `load()`: read the whole jar text through `js_ls_get`, then parse
/// it exactly as the host does. A read that fills the scratch is possibly
/// truncated, so its last (partial) line is dropped before parsing.
#[cfg(target_arch = "wasm32")]
fn ls_load() -> Vec<(String, String)> {
    let mut buf = [0u8; LS_SCRATCH_CAP];
    // SAFETY: `buf` is a live, writable region of LS_SCRATCH_CAP bytes and the
    // JS bridge writes no more than `cap` bytes into it during this call.
    let n = unsafe { js_ls_get(buf.as_mut_ptr(), LS_SCRATCH_CAP) }.min(LS_SCRATCH_CAP);
    let used = if n == LS_SCRATCH_CAP {
        buf[..n]
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |i| i + 1)
    } else {
        n
    };
    match std::str::from_utf8(&buf[..used]) {
        Ok(text) => parse(text),
        Err(_) => Vec::new(),
    }
}

/// Build the exact same jar text the host `save()` builds and hand it to the
/// localStorage bridge. Kept separate so the host loop below stays verbatim.
#[cfg(target_arch = "wasm32")]
fn ls_save(pairs: &[(String, String)]) {
    let mut text = String::new();
    for (k, v) in pairs {
        text.push_str(k);
        text.push('=');
        text.push_str(v);
        text.push('\n');
    }
    // SAFETY: `text.as_ptr()` is readable for `text.len()` bytes and the JS
    // bridge copies that range synchronously before this call returns.
    unsafe { js_ls_set(text.as_ptr(), text.len()) };
}

/// The local cookie jar.
#[derive(Debug, Clone)]
pub struct CookieStore {
    path: PathBuf,
}

/// Resolve one configured cookie path into its candidates, primary first.
///
/// - absolute `path` → `[path]` (the test seam; never redirected),
/// - relative `path` → `cwd/path`, then `exe_dir/path` (deduped when the
///   process was started from the executable's own directory),
/// - no cwd and no exe dir (neither should happen) → `[path]`.
///
/// Pure so the resolution rule is unit-testable without touching the
/// filesystem or the process cwd.
// Host path resolution is not reached on wasm; keep the host tests compiling
// on every target without emitting a dead-code warning in the wasm build.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn cookie_candidates(path: &Path, cwd: Option<&Path>, exe_dir: Option<&Path>) -> Vec<PathBuf> {
    if path.is_absolute() {
        return vec![path.to_path_buf()];
    }
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(cwd) = cwd {
        out.push(cwd.join(path));
    }
    if let Some(dir) = exe_dir {
        let cand = dir.join(path);
        if !out.contains(&cand) {
            out.push(cand);
        }
    }
    if out.is_empty() {
        out.push(path.to_path_buf());
    }
    out
}

impl CookieStore {
    /// A store backed by `path` (tests inject a scratch file).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The deploy store: `nfm-cookies.txt`, cwd first, then beside the
    /// executable.
    pub fn default_path() -> Self {
        Self::new("nfm-cookies.txt")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The configured path resolved into its read/write candidates.
    #[cfg(not(target_arch = "wasm32"))]
    fn candidates(&self) -> Vec<PathBuf> {
        let cwd = std::env::current_dir().ok();
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        cookie_candidates(&self.path, cwd.as_deref(), exe_dir.as_deref())
    }

    /// Parse the first readable candidate; a missing jar is empty.
    #[cfg(not(target_arch = "wasm32"))]
    fn load(&self) -> Vec<(String, String)> {
        for path in self.candidates() {
            if let Ok(text) = std::fs::read_to_string(path) {
                return parse(&text);
            }
        }
        Vec::new()
    }

    /// The web `load()`: the whole jar text is one localStorage entry, read
    /// through `js_ls_get` and parsed exactly like the host file jar.
    #[cfg(target_arch = "wasm32")]
    fn load(&self) -> Vec<(String, String)> {
        ls_load()
    }

    /// `savecookie(key, value)` — upsert the `key=value` line.
    /// I/O failures are swallowed like the source's `catch (Exception)`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&mut self, key: &str, value: &str) {
        let mut pairs = self.load();
        if let Some(slot) = pairs.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value.to_string();
        } else {
            pairs.push((key.to_string(), value.to_string()));
        }
        let mut text = String::new();
        for (k, v) in &pairs {
            text.push_str(k);
            text.push('=');
            text.push_str(v);
            text.push('\n');
        }
        for path in self.candidates() {
            if flush_at(&path, &text) {
                return;
            }
        }
    }

    /// The web `save()`: identical upsert + jar-text build, handed to
    /// `js_ls_set` instead of `flush_at`. Storage failures are swallowed in JS.
    #[cfg(target_arch = "wasm32")]
    pub fn save(&mut self, key: &str, value: &str) {
        let mut pairs = self.load();
        if let Some(slot) = pairs.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value.to_string();
        } else {
            pairs.push((key.to_string(), value.to_string()));
        }
        ls_save(&pairs);
    }

    /// `readcookie(key)` — the value as `i32`, −1 on missing/garbage.
    pub fn read(&self, key: &str) -> i32 {
        for (k, v) in self.load() {
            if k == key {
                return v.parse::<i32>().unwrap_or(-1);
            }
        }
        -1
    }
}

/// Parse `key=value` lines (the jar's on-disk shape).
fn parse(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            pairs.push((k.to_string(), v.to_string()));
        }
    }
    pairs
}

/// Write `text` to `path` through a sibling tmp file + rename; `false` when
/// the directory is not writable, so the caller can try the next candidate.
#[cfg(not(target_arch = "wasm32"))]
fn flush_at(path: &Path, text: &str) -> bool {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, text).is_err() {
        return false;
    }
    if std::fs::rename(&tmp, path).is_ok() {
        true
    } else {
        let _ = std::fs::remove_file(&tmp);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> (CookieStore, PathBuf) {
        let path =
            std::env::temp_dir().join(format!("nfm-cookies-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        (CookieStore::new(&path), path)
    }

    #[test]
    fn candidates_prefer_cwd_then_the_exe_dir() {
        let cwd = Path::new("/work/game");
        let exe = Path::new("/opt/nfm");
        assert_eq!(
            cookie_candidates(Path::new("nfm-cookies.txt"), Some(cwd), Some(exe)),
            vec![
                PathBuf::from("/work/game/nfm-cookies.txt"),
                PathBuf::from("/opt/nfm/nfm-cookies.txt")
            ]
        );
    }

    #[test]
    fn candidates_dedupe_when_cwd_is_the_exe_dir() {
        let dir = Path::new("/opt/nfm");
        assert_eq!(
            cookie_candidates(Path::new("nfm-cookies.txt"), Some(dir), Some(dir)),
            vec![PathBuf::from("/opt/nfm/nfm-cookies.txt")]
        );
    }

    #[test]
    fn candidates_pin_absolute_paths_and_survive_missing_dirs() {
        // the test seam: an injected absolute path never redirects
        assert_eq!(
            cookie_candidates(
                Path::new("/tmp/x-cookies.txt"),
                Some(Path::new("/work")),
                Some(Path::new("/opt/nfm"))
            ),
            vec![PathBuf::from("/tmp/x-cookies.txt")]
        );
        // relative path with no cwd/exe dir still yields the bare path
        assert_eq!(
            cookie_candidates(Path::new("nfm-cookies.txt"), None, None),
            vec![PathBuf::from("nfm-cookies.txt")]
        );
        // cwd missing: the exe dir is the only candidate
        assert_eq!(
            cookie_candidates(
                Path::new("nfm-cookies.txt"),
                None,
                Some(Path::new("/opt/nfm"))
            ),
            vec![PathBuf::from("/opt/nfm/nfm-cookies.txt")]
        );
    }

    #[test]
    fn round_trip_and_upsert() {
        let (mut store, path) = scratch("roundtrip");
        store.save("unlocked", "5");
        store.save("usercar", "3");
        assert_eq!(store.read("unlocked"), 5);
        assert_eq!(store.read("usercar"), 3);

        // upsert replaces in place (one line, last wins)
        store.save("unlocked", "7");
        assert_eq!(store.read("unlocked"), 7);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("unlocked=").count(), 1);
        assert!(text.contains("usercar=3\n"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn minus_one_default() {
        let (store, path) = scratch("default");
        // missing file
        assert_eq!(store.read("unlocked"), -1);
        // missing key beside a present one
        let mut store = store;
        store.save("unlocked", "4");
        assert_eq!(store.read("gameprfact"), -1);
        // garbage value (the Integer.valueOf failure path)
        store.save("unlocked", "lots");
        assert_eq!(store.read("unlocked"), -1);
        // empty value
        store.save("unlocked", "");
        assert_eq!(store.read("unlocked"), -1);
        let _ = std::fs::remove_file(&path);
    }
}
