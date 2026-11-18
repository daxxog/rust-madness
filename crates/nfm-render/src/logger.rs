//! `logger` — the play path's print sink, with a debug build's on-disk copy.
//!
//! Play mode's boot/audio/error lines keep printing exactly as before. In a
//! **debug** build they are also appended to `nfm.log` in the working
//! directory (rotated once past 1 MiB to `nfm.log.1`), which is the file the
//! local debug server serves as `/log` and `/log/tail`.
//!
//! A release build compiles the file writer out entirely: [`line()`] and
//! [`error`] are the same `println!`/`eprintln!` the call sites used before.
//!
//! Deliberately not a logging framework: the scope is play mode's own print
//! sites (boot, audio, frames presented, errors) plus the pad's open/connect
//! lines, which only the play path can reach. Headless modes and the mirror
//! suites never call it.

#[cfg(debug_assertions)]
use std::io::Write;

/// The served log's file name, resolved in the working directory.
#[cfg(debug_assertions)]
pub const LOG_NAME: &str = "nfm.log";

/// Rotation threshold: past this the file moves aside to `nfm.log.1`.
#[cfg(debug_assertions)]
const ROTATE_BYTES: u64 = 1 << 20;

/// One informational line: stdout as before, plus (debug only) the log file.
pub fn line(msg: &str) {
    println!("{msg}");
    #[cfg(debug_assertions)]
    append(msg);
}

/// One error line: stderr as before, plus (debug only) the same log file.
pub fn error(msg: &str) {
    eprintln!("{msg}");
    #[cfg(debug_assertions)]
    append(msg);
}

#[cfg(debug_assertions)]
fn append(msg: &str) {
    append_to(std::path::Path::new(LOG_NAME), msg);
}

/// Append one stamped line, rotating a >1 MiB file once. Every failure is
/// swallowed: a missing log file must never break play.
#[cfg(debug_assertions)]
fn append_to(path: &std::path::Path, msg: &str) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    if std::fs::metadata(path).is_ok_and(|m| m.len() > ROTATE_BYTES) {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "[{stamp}] {msg}");
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nfm-logger-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn append_writes_one_stamped_line_per_call() {
        let dir = scratch("lines");
        let path = dir.join("nfm.log");
        append_to(&path, "boot: stage 1");
        append_to(&path, "audio: silent");
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with('['), "timestamped: {}", lines[0]);
        assert!(lines[0].ends_with(" boot: stage 1"));
        assert!(lines[1].ends_with(" audio: silent"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_moves_the_old_file_aside() {
        let dir = scratch("rotate");
        let path = dir.join("nfm.log");
        std::fs::write(&path, vec![b'x'; ROTATE_BYTES as usize + 1]).unwrap();
        append_to(&path, "after rotation");
        assert!(dir.join("nfm.log.1").is_file(), "old file moved aside");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.trim_end().ends_with("after rotation"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
