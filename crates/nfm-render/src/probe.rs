//! `probe` — the debug-build first-launch pad probe (the D3 verdict).
//!
//! **DEBUG BUILDS ONLY** (`main.rs` declares the module under
//! `#[cfg(debug_assertions)]`), and inside a debug build only until
//! `nfm-probe.done` exists beside the executable.
//!
//! Why it exists: on the Deck there is no shell to read `/dev/input` from,
//! and the rear paddles (`BTN_TRIGGER_HAPPY1..4`) are not named gilrs
//! buttons — they arrive as `Button::Unknown` with a raw evdev code. The
//! probe runs ON the real window, through the SAME `present` path the game
//! loop uses, for 120 seconds before the game boots. It shows:
//!
//! * every gilrs device name,
//! * every button event the API delivers — name + raw code, most recent
//!   first (press every Deck control and watch what arrives),
//! * a countdown, and the "press Start to finish early" affordance
//!   (Start, Enter/Escape or a left click all end it).
//!
//! On exit it writes, beside the executable:
//!
//! * `nfm-probe.log`  — every gilrs event, one line each (bounded to 100k
//!   lines; axis events are included, so a trigger pull paints a run);
//! * `nfm-probe.done` — the small verdict summary: devices, all button
//!   names seen, and whether `BTN_TRIGGER_HAPPY1..4` were seen. **The
//!   marker's existence is what skips the probe on later launches** —
//!   delete it (or start the exe from a directory whose exe-dir copy is
//!   absent) to run the probe again.
//!
//! Integration with play mode's boot order: the window opens, the debug
//! server starts, this probe runs on that window, THEN the play gamepad
//! adapter opens (the probe owns its own gilrs instance and drops it before
//! returning, so the two never fight over a device), then audio, then the
//! normal realtime loop. The probe touches no shell/xt/game state — it only
//! paints frames and reads the pad.
//!
//! If the executable directory cannot be resolved the probe is skipped
//! entirely: without a marker location there is no way to run it once.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gilrs::{Button, EventType, GilrsBuilder};
use nfm_core::canvas::Canvas;
use nfm_core::host::RealTimeHost;

use crate::logger;
use crate::window::{HEIGHT, WIDTH, Window};

/// The probe's fixed window.
pub const SECONDS: u64 = 120;
/// The skip marker; its existence means "this machine has been probed".
pub const MARKER: &str = "nfm-probe.done";
/// The full event log written at exit.
pub const EVENT_LOG: &str = "nfm-probe.log";

/// Most-recent-first cap for the live list and the log's line count.
const RECENT_CAP: usize = 36;
const LOG_LINE_CAP: usize = 100_000;
/// Button lines that fit on screen under the header.
const SCREEN_LINES: usize = 15;
/// The probe's frame pace. The full countdown runs even without a pad: the
/// screen is also the render-path proof.
const FRAME_MS: u64 = 16;

/// Run the probe if this is the first launch, writing both artifacts.
///
/// Called only from the debug play path, before the play gamepad adapter
/// is constructed.
pub fn run(win: &mut Window) {
    let Some(dir) = exe_dir() else {
        logger::line("probe: cannot resolve the executable directory; skipping");
        return;
    };
    let marker = dir.join(MARKER);
    if marker.exists() {
        logger::line(&format!(
            "probe: {} present; skipping (delete it to probe again)",
            marker.display()
        ));
        return;
    }
    logger::line(&format!(
        "probe: first launch — {SECONDS}s in-window pad probe (Start/Enter/click ends early)"
    ));
    let report = run_loop(win);
    match write_artifacts(&dir, &report) {
        Ok(()) => logger::line(&format!(
            "probe: wrote {} ({} event line(s)) and {} — later launches skip the probe",
            dir.join(EVENT_LOG).display(),
            report.log.len(),
            dir.join(MARKER).display()
        )),
        Err(e) => logger::error(&e),
    }
    logger::line(&format!(
        "probe: done — {} device(s), {} button name(s), rear paddles {}",
        report.devices.len(),
        report.buttons.len(),
        if report.rear.iter().any(|seen| *seen) {
            "SEEN"
        } else {
            "not seen"
        }
    ));
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// The probe's collected evidence.
struct Report {
    started_unix_ms: u128,
    duration_ms: u128,
    devices: Vec<String>,
    gilrs_error: Option<String>,
    /// `name raw-code` per distinct button event name seen.
    buttons: BTreeSet<String>,
    /// BTN_TRIGGER_HAPPY1..4 seen (raw evdev codes 0x2c0..0x2c3).
    rear: [bool; 4],
    /// Every gilrs event, one line each.
    log: Vec<String>,
    /// Why the loop ended (countdown, an early finish, or the close box).
    end: &'static str,
}

/// The window loop: drain the pad, repaint the live evidence, present.
fn run_loop(win: &mut Window) -> Report {
    let clock = Instant::now();
    let started_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    let (mut gilrs, devices, gilrs_error) =
        match GilrsBuilder::new().with_force_feedback(false).build() {
            Ok(gilrs) => {
                let devices = gilrs
                    .gamepads()
                    .map(|(id, pad)| format!("{}: {}", usize::from(id), pad.name()))
                    .collect();
                (Some(gilrs), devices, None)
            }
            Err(e) => (None, Vec::new(), Some(e.to_string())),
        };

    let mut report = Report {
        started_unix_ms,
        duration_ms: 0,
        devices,
        gilrs_error,
        buttons: BTreeSet::new(),
        rear: [false; 4],
        log: Vec::new(),
        end: "countdown elapsed",
    };
    let mut recent: Vec<String> = Vec::new();
    let mut finished = false;
    let mut end = "countdown elapsed";
    // A click/Enter held from launching the app must not count as a skip;
    // initialising the edge state from the first sample makes only a NEW
    // press end the probe.
    let mut prev_skip = win.skip_requested();
    let deadline = Duration::from_secs(SECONDS);

    while clock.elapsed() < deadline {
        if win.wants_exit() {
            end = "window closed";
            break;
        }
        let mut events = Vec::new();
        if let Some(gilrs) = gilrs.as_mut() {
            while let Some(ev) = gilrs.next_event() {
                events.push(ev);
            }
        }
        for ev in &events {
            let elapsed = clock.elapsed().as_millis();
            if report.log.len() < LOG_LINE_CAP {
                report
                    .log
                    .push(format!("{elapsed:>7} ms  {}", describe(ev)));
            }
            match ev.event {
                EventType::ButtonPressed(button, code) => {
                    let name = format!("{button:?}  raw 0x{:04x}", raw16(code));
                    report.buttons.insert(name.clone());
                    if let Some(i) = rear_index_packed(code.into_u32()) {
                        report.rear[i] = true;
                    }
                    recent.insert(0, format!("DOWN  {name}"));
                    recent.truncate(RECENT_CAP);
                    if button == Button::Start {
                        finished = true;
                        end = "start button";
                    }
                }
                EventType::ButtonReleased(button, code) => {
                    recently(
                        &mut recent,
                        format!("up    {button:?}  raw 0x{:04x}", raw16(code)),
                    );
                }
                EventType::Connected => {
                    let name = gilrs
                        .as_ref()
                        .and_then(|g| g.connected_gamepad(ev.id))
                        .map_or_else(|| "unknown".to_string(), |p| p.name().to_string());
                    let line = format!("{}: {name}", usize::from(ev.id));
                    if !report.devices.contains(&line) {
                        report.devices.push(line);
                    }
                    recently(
                        &mut recent,
                        format!("connected {name} (id {})", usize::from(ev.id)),
                    );
                }
                EventType::Disconnected => {
                    recently(
                        &mut recent,
                        format!("disconnected id {}", usize::from(ev.id)),
                    );
                }
                _ => {}
            }
        }

        let skip = win.skip_requested();
        if skip && !prev_skip {
            finished = true;
            end = "start/enter/click";
        }
        prev_skip = skip;

        let left = deadline.saturating_sub(clock.elapsed()).as_secs();
        let cv = draw(&report, &recent, left);
        win.present(&cv);
        std::thread::sleep(Duration::from_millis(FRAME_MS));
        if finished {
            break;
        }
    }

    report.duration_ms = clock.elapsed().as_millis();
    report.end = end;
    logger::line(&format!(
        "probe: loop ended after {} ms ({end})",
        report.duration_ms
    ));
    report
}

fn recently(recent: &mut Vec<String>, line: String) {
    recent.insert(0, line);
    recent.truncate(RECENT_CAP);
}

/// One event as a single log line (the raw code is the D3 evidence).
fn describe(ev: &gilrs::Event) -> String {
    let pad = usize::from(ev.id);
    match ev.event {
        EventType::ButtonPressed(b, c) => {
            format!(
                "pad{pad} DOWN  {b:?}  raw 0x{:04x}  packed 0x{:08x}",
                raw16(c),
                c.into_u32()
            )
        }
        EventType::ButtonReleased(b, c) => {
            format!(
                "pad{pad} up    {b:?}  raw 0x{:04x}  packed 0x{:08x}",
                raw16(c),
                c.into_u32()
            )
        }
        EventType::ButtonChanged(b, v, c) => {
            format!("pad{pad} value {b:?} {v:+.3}  raw 0x{:04x}", raw16(c))
        }
        EventType::AxisChanged(a, v, c) => {
            format!("pad{pad} axis  {a:?} {v:+.3}  raw 0x{:04x}", raw16(c))
        }
        EventType::Connected => format!("pad{pad} connected"),
        EventType::Disconnected => format!("pad{pad} disconnected"),
        EventType::Dropped => format!("pad{pad} dropped"),
        _ => format!("pad{pad} other"),
    }
}

/// The low 16 bits of a packed gilrs code (`kind << 16 | code`).
fn raw16(code: gilrs::ev::Code) -> u32 {
    code.into_u32() & 0xffff
}

/// A rear-paddle slot from the packed evdev code. EV_KEY (kind 1) rows
/// 0x2c0..0x2c3 are `BTN_TRIGGER_HAPPY1..4`.
fn rear_index_packed(packed: u32) -> Option<usize> {
    const EV_KEY: u32 = 1;
    if packed >> 16 != EV_KEY {
        return None;
    }
    match (packed & 0xffff) as u16 {
        0x2c0 => Some(0),
        0x2c1 => Some(1),
        0x2c2 => Some(2),
        0x2c3 => Some(3),
        _ => None,
    }
}

/// Paint one probe frame: the xtfont atlas at its two real sizes (13 for the
/// title, 11 for the body — the atlas has exactly the sizes the applet asks
/// for), plain left-aligned text; no xt/Medium state is involved.
fn draw(report: &Report, recent: &[String], left: u64) -> Canvas {
    let mut cv = Canvas::new(WIDTH, HEIGHT);
    cv.fill([0, 0, 0]);
    cv.fill_rect(0, 0, WIDTH as i32, 1, [40, 80, 140]);

    let title = [160, 200, 255];
    let body = [210, 210, 210];
    let good = [120, 220, 120];
    let hot = [255, 235, 140];
    let dim = [130, 130, 145];

    cv.draw_string("NFM first-launch pad probe", 12, 24, title, 13);
    cv.draw_string(
        &format!("{left:>3}s left — press Start (or Enter / click) to finish early"),
        12,
        46,
        body,
        11,
    );
    match &report.gilrs_error {
        Some(e) => cv.draw_string(&format!("gilrs unavailable: {e}"), 12, 68, hot, 11),
        None => cv.draw_string(
            &format!("gilrs: {} device(s)", report.devices.len()),
            12,
            68,
            good,
            11,
        ),
    }

    let mut y = 90;
    for device in report.devices.iter().take(4) {
        cv.draw_string(&clip(device, 80), 24, y, good, 11);
        y += 15;
    }
    cv.draw_string("button events received (newest first):", 12, y + 2, dim, 11);
    y += 20;
    if recent.is_empty() {
        cv.draw_string(
            "press every Deck control — nothing received yet",
            24,
            y,
            dim,
            11,
        );
    }
    for line in recent.iter().take(SCREEN_LINES) {
        cv.draw_string(&clip(line, 62), 24, y, hot, 11);
        y += 15;
    }
    cv.draw_string(
        "writes nfm-probe.log + nfm-probe.done beside the executable",
        12,
        HEIGHT as i32 - 10,
        dim,
        11,
    );
    cv
}

/// Clamp a line to `max` chars (the 550px frame only fits so much).
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max.saturating_sub(3)).collect::<String>() + "..."
}

/// Write the evidence files. A failure is returned for the caller to log and
/// never fatal: play continues either way (the probe's job is evidence, not
/// a gate). The marker is only written once the event log landed — a half
/// write must not suppress the probe forever.
fn write_artifacts(dir: &Path, report: &Report) -> Result<(), String> {
    let log_path = dir.join(EVENT_LOG);
    let mut text = String::new();
    for line in &report.log {
        text.push_str(line);
        text.push('\n');
    }
    std::fs::write(&log_path, text)
        .map_err(|e| format!("probe: cannot write {}: {e}", log_path.display()))?;
    let marker = dir.join(MARKER);
    std::fs::write(&marker, summary(report))
        .map_err(|e| format!("probe: cannot write {}: {e}", marker.display()))?;
    Ok(())
}

/// The `nfm-probe.done` text: the D3 verdict artifact.
fn summary(report: &Report) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "nfm-probe.done — the D3 on-Deck pad verdict (debug build)"
    );
    let _ = writeln!(s, "started_unix_ms: {}", report.started_unix_ms);
    let _ = writeln!(s, "duration_ms: {} ({})", report.duration_ms, report.end);
    let _ = writeln!(s, "devices: {}", report.devices.len());
    for device in &report.devices {
        let _ = writeln!(s, "  {device}");
    }
    if let Some(e) = &report.gilrs_error {
        let _ = writeln!(s, "gilrs_error: {e}");
    }
    let _ = writeln!(s, "button names seen: {}", report.buttons.len());
    for button in &report.buttons {
        let _ = writeln!(s, "  {button}");
    }
    let _ = writeln!(
        s,
        "rear paddles (BTN_TRIGGER_HAPPY1..4, EV_KEY 0x2c0..0x2c3):"
    );
    for (i, seen) in report.rear.iter().enumerate() {
        let _ = writeln!(
            s,
            "  BTN_TRIGGER_HAPPY{} (0x{:x}): {}",
            i + 1,
            0x2c0 + i,
            if *seen { "SEEN" } else { "not seen" }
        );
    }
    let _ = writeln!(
        s,
        "verdict: {}",
        if report.rear.iter().any(|seen| *seen) {
            "rear paddles reach gilrs as Unknown + raw evdev codes; match \
             gamepad.rs's REAR table to the order above"
        } else {
            "no BTN_TRIGGER_HAPPY events observed: the rear paddles may be \
             consumed by Steam Input (Steam's controller layout) or not exposed"
        }
    );
    let _ = writeln!(s, "delete this file to run the probe again");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rear_paddle_codes_map_to_the_four_slots() {
        assert_eq!(rear_index_packed(0x1_02c0), Some(0));
        assert_eq!(rear_index_packed(0x1_02c1), Some(1));
        assert_eq!(rear_index_packed(0x1_02c2), Some(2));
        assert_eq!(rear_index_packed(0x1_02c3), Some(3));
        assert_eq!(
            rear_index_packed(0x3_02c0),
            None,
            "ABS kinds are not buttons"
        );
        assert_eq!(rear_index_packed(0x1_02c4), None, "beyond the four");
        assert_eq!(rear_index_packed(0x1_0130), None, "BTN_SOUTH is named");
    }

    #[test]
    fn summary_lists_devices_buttons_and_the_rear_verdict() {
        let report = Report {
            started_unix_ms: 1,
            duration_ms: 2,
            devices: vec!["0: Microsoft X-Box 360 pad".to_string()],
            gilrs_error: None,
            buttons: [
                "South  raw 0x0130".to_string(),
                "Start  raw 0x013b".to_string(),
            ]
            .into_iter()
            .collect(),
            rear: [true, false, false, false],
            log: vec![],
            end: "countdown elapsed",
        };
        let text = summary(&report);
        assert!(text.contains("devices: 1"));
        assert!(text.contains("0: Microsoft X-Box 360 pad"));
        assert!(text.contains("South  raw 0x0130"));
        assert!(text.contains("Start  raw 0x013b"));
        assert!(text.contains("BTN_TRIGGER_HAPPY1 (0x2c0): SEEN"));
        assert!(text.contains("BTN_TRIGGER_HAPPY4 (0x2c3): not seen"));
        assert!(text.contains("rear paddles reach gilrs"));
        assert!(text.contains("delete this file"));
    }

    #[test]
    fn lines_are_clipped_to_the_frame() {
        let long = "x".repeat(200);
        let clipped = clip(&long, 20);
        assert_eq!(clipped.chars().count(), 20);
        assert!(clipped.ends_with("..."));
        assert_eq!(clip("short", 20), "short");
    }

    /// The two artifacts land beside the executable's directory: the full
    /// event log and the D3 summary that doubles as the skip marker.
    #[test]
    fn artifacts_are_written_where_the_marker_will_be_found() {
        let dir = std::env::temp_dir().join(format!("nfm-probe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let report = Report {
            started_unix_ms: 1,
            duration_ms: 2,
            devices: vec!["0: Deck pad".to_string()],
            gilrs_error: None,
            buttons: ["South  raw 0x0130".to_string()].into_iter().collect(),
            rear: [false, false, false, true],
            log: vec!["    10 ms  pad0 DOWN  South  raw 0x0130".to_string()],
            end: "start/enter/click",
        };
        write_artifacts(&dir, &report).expect("artifacts write");
        let log = std::fs::read_to_string(dir.join(EVENT_LOG)).unwrap();
        assert!(log.contains("DOWN  South"));
        let done = std::fs::read_to_string(dir.join(MARKER)).unwrap();
        assert!(done.contains("devices: 1"));
        assert!(done.contains("BTN_TRIGGER_HAPPY4 (0x2c3): SEEN"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
