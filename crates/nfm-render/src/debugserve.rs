//! `debugserve` — the Steam Deck's read-only evidence server.
//!
//! **DEBUG BUILDS ONLY.** `main.rs` declares this module under
//! `#[cfg(debug_assertions)]`, so a release binary contains none of it: no
//! listener, no probe, no routes, no strings. On the Deck there is no
//! ergonomic shell, so a debug play session collects its own evidence and
//! serves it over the LAN — the design the user's zig reference repo
//! (`steam-deck-agent`'s `logserver.zig` / `diag.zig`) proved:
//!
//! * **read-only** — every route is a GET that reads an existing file or the
//!   shared frame/keys snapshot; nothing is ever written;
//! * **bounded** — whole-file reads, tails and the request head all have
//!   explicit caps (a curious client cannot pin RAM or stream forever);
//! * **explicitly enumerated** — the route table below is the complete
//!   list; `/env` reports only an allow-listed set of display/session
//!   variables (and `XAUTHORITY` as a *path*, never its contents).
//!
//! ## Routes (GET only)
//!
//! | route       | body                                                        |
//! |-------------|-------------------------------------------------------------|
//! | `/`         | index page listing these routes                             |
//! | `/log`      | `nfm.log` (whole; last 4 MiB if larger)                     |
//! | `/log/tail` | `nfm.log` tail (`?bytes=N`, default 64 KiB, cap 4 MiB)      |
//! | `/probe.log`| `nfm-probe.log`, one line per pad event (cap 4 MiB)         |
//! | `/frame.bmp`| the latest presented 550x400 canvas as a BMP                |
//! | `/keys`     | the WATCH table state + recent key/pad edges                |
//! | `/cookies`  | `nfm-cookies.txt` (cap 64 KiB)                              |
//! | `/env`      | the env allow-list + build/audio/pad facts                  |
//!
//! Anything else is a 404; a non-GET method is a 405.
//!
//! ## Shape
//!
//! A plain `std::net::TcpListener` on a fixed port (8085), bound to
//! 0.0.0.0, on its own `std::thread`. One request per connection, one
//! response, `Connection: close`. If the port is taken the server logs one
//! line and is skipped entirely — it must never block play. The thread polls
//! with a 50 ms tick so dropping [`DebugServer`] stops it promptly (a
//! blocking `accept` would ignore the stop flag until a client connected).
//!
//! ## Frame + keys sharing
//!
//! [`DebugShare`] is the only state the server thread cannot derive from a
//! file: the play loop's latest presented RGB canvas, the `/keys` text
//! snapshot (refreshed on present/poll/pump), the pad inventory and the
//! audio route summary. `window::Window` owns the `Arc`, writes it on the
//! game thread with short-lived locks, and the server reads it with the same
//! discipline. A mutex plus a 660 KB copy per present is cheap next to the
//! frame's own pixel work.

use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nfm_core::canvas::Canvas;

use crate::logger;
use crate::window::{HEIGHT, WIDTH};

/// The fixed debug port (never configurable: one well-known LAN endpoint).
pub const PORT: u16 = 8085;

/// `/log` and `/probe.log` whole-file cap: beyond it the *last* 4 MiB is
/// served with a truncation marker line.
pub const WHOLE_CAP: usize = 4 << 20;
/// `/log/tail` default window.
pub const TAIL_DEFAULT: usize = 64 << 10;
/// `/log/tail` hard cap (`?bytes=N` above it is a 413).
pub const TAIL_CAP: usize = 4 << 20;
/// `/cookies` cap (the jar is a handful of lines).
const COOKIE_CAP: usize = 64 << 10;
/// Request head cap: anything past this is not a request we serve.
const REQUEST_CAP: usize = 8 << 10;
/// `/env` value cap (paths/user names; truncation only guards abuse).
const VALUE_CAP: usize = 300;
/// Stop-tick sleep: far below any perceptual threshold, far above the cost.
const ACCEPT_TICK: Duration = Duration::from_millis(50);

/// `/env`'s complete allow-list, in output order. Nothing secret can live
/// here (all display/session/steam markers); the list is explicit so future
/// additions stay deliberate. `XAUTHORITY` is a path — the cookie file it
/// names is never opened or printed.
const ENV_ALLOW: [&str; 8] = [
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_TYPE",
    "DISPLAY",
    "XAUTHORITY",
    "STEAM_GAME",
    "STEAM_DECK",
    "GAMESCOPE_WAYLAND_MODE",
];

/// The read-only snapshot the play thread publishes and the server serves.
#[derive(Default)]
pub struct DebugShare {
    /// The latest presented canvas (RGB triples, row-major) — `/frame.bmp`.
    pub frame: Mutex<Option<Vec<u8>>>,
    /// The `/keys` text snapshot (WATCH rows + recent edges).
    pub keys: Mutex<String>,
    /// The pad inventory as `id: name` lines — `/env`.
    pub pads: Mutex<Vec<String>>,
    /// The opened audio route's one-line summary — `/env`.
    pub audio: Mutex<String>,
}

/// The running server: drop to stop and join the thread.
pub struct DebugServer {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl DebugServer {
    /// Bind and serve, or log one line and return `None`. Never blocks play:
    /// binding is done here, before the thread starts, so a taken port is
    /// reported synchronously.
    pub fn start(share: Arc<DebugShare>) -> Option<DebugServer> {
        let listener = match TcpListener::bind(("0.0.0.0", PORT)) {
            Ok(l) => l,
            Err(e) => {
                logger::error(&format!(
                    "debug server: cannot bind 0.0.0.0:{PORT} ({e}); continuing without it"
                ));
                return None;
            }
        };
        // Nonblocking accept + a tick is the graceful-stop shape (see the
        // module doc); the cost only exists between connections.
        let _ = listener.set_nonblocking(true);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let join = std::thread::Builder::new()
            .name("nfm-debug".to_string())
            .spawn(move || serve(&listener, &share, &thread_stop))
            .map_err(|e| {
                logger::error(&format!(
                    "debug server: cannot spawn thread ({e}); not serving"
                ));
            })
            .ok()?;
        logger::line(&format!(
            "debug server: http://0.0.0.0:{PORT}/ (read-only, debug build only)"
        ));
        Some(DebugServer {
            stop,
            join: Some(join),
        })
    }
}

impl Drop for DebugServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// The accept loop: one request per connection, a poll tick between them.
fn serve(listener: &TcpListener, share: &Arc<DebugShare>, stop: &AtomicBool) {
    let paths = Paths::live();
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                // A single misbehaving client must not take the thread down,
                // and a mid-response failure is worth one log line.
                if let Err(e) = handle_conn(stream, share, &paths) {
                    logger::error(&format!("debug server: connection error ({e})"));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_TICK);
            }
            Err(_) => std::thread::sleep(ACCEPT_TICK),
        }
    }
}

fn handle_conn(
    mut stream: TcpStream,
    share: &Arc<DebugShare>,
    paths: &Paths,
) -> std::io::Result<()> {
    // BSD/macOS accepted sockets inherit the listener's O_NONBLOCK; put the
    // connection back into blocking mode or a large response (the 660 KB
    // frame BMP) dies half-written with EAGAIN.
    stream.set_nonblocking(false)?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() >= REQUEST_CAP {
            break;
        }
    }
    let response = match parse_request(&buf) {
        Some((method, target)) => route(share, paths, &method, &target),
        None => Response::text(400, "bad request line\n".to_string()),
    };
    write_response(&mut stream, &response)
}

/// `GET /path?query HTTP/1.1` → `(method, target)`. Only the request line is
/// looked at; everything else is ignored.
fn parse_request(buf: &[u8]) -> Option<(String, String)> {
    let text = std::str::from_utf8(buf).ok()?;
    let line = text.lines().next()?;
    let mut parts = line.split_ascii_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    if method.is_empty() || !target.starts_with('/') {
        return None;
    }
    Some((method.to_string(), target.to_string()))
}

/// The filesystem routes' paths (injectable so the dispatcher is testable).
struct Paths {
    log: PathBuf,
    probe: PathBuf,
    cookies: Vec<PathBuf>,
}

impl Paths {
    /// The live resolution: `nfm.log` in the working directory (where the
    /// logger writes), the probe artifacts beside the executable (where the
    /// probe writes), cookies cwd-first then beside the executable (the
    /// `CookieStore` fallback).
    fn live() -> Paths {
        let cwd = std::env::current_dir().unwrap_or_default();
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        let probe = exe_dir.as_deref().unwrap_or(&cwd).join("nfm-probe.log");
        let mut cookies = vec![cwd.join("nfm-cookies.txt")];
        if let Some(dir) = &exe_dir {
            let cand = dir.join("nfm-cookies.txt");
            if !cookies.contains(&cand) {
                cookies.push(cand);
            }
        }
        Paths {
            log: cwd.join(logger::LOG_NAME),
            probe,
            cookies,
        }
    }
}

/// One canned HTTP response.
struct Response {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl Response {
    fn text(status: u16, body: String) -> Response {
        Response {
            status,
            content_type: "text/plain; charset=utf-8",
            body: body.into_bytes(),
        }
    }

    fn html(status: u16, body: String) -> Response {
        Response {
            status,
            content_type: "text/html; charset=utf-8",
            body: body.into_bytes(),
        }
    }

    fn bmp(body: Vec<u8>) -> Response {
        Response {
            status: 200,
            content_type: "image/bmp",
            body,
        }
    }
}

/// The complete route table (see the module doc). Pure except for the file
/// and share reads, so the dispatcher is unit-tested on canned requests.
fn route(share: &Arc<DebugShare>, paths: &Paths, method: &str, target: &str) -> Response {
    if method != "GET" {
        return Response::text(405, "GET only\n".to_string());
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    match path {
        "/" => Response::html(200, INDEX.to_string()),
        "/log" => Response::text(
            200,
            text_body(
                read_capped(&paths.log, WHOLE_CAP),
                "(nfm.log has no lines yet)\n",
            ),
        ),
        "/log/tail" => match tail_bytes(query) {
            Ok(n) => Response::text(
                200,
                text_body(read_tail(&paths.log, n), "(nfm.log has no lines yet)\n"),
            ),
            Err(resp) => resp,
        },
        "/probe.log" => Response::text(
            200,
            text_body(
                read_capped(&paths.probe, WHOLE_CAP),
                "(nfm-probe.log not found — the probe has not run)\n",
            ),
        ),
        "/frame.bmp" => Response::bmp(frame_bmp(share)),
        "/keys" => {
            let body = share
                .keys
                .lock()
                .map(|g| g.clone())
                .unwrap_or_else(|_| String::new());
            Response::text(
                200,
                if body.is_empty() {
                    "(no WATCH snapshot yet — the loop has not presented a frame)\n".to_string()
                } else {
                    body
                },
            )
        }
        "/cookies" => Response::text(
            200,
            text_body(
                read_first(&paths.cookies, COOKIE_CAP),
                "(nfm-cookies.txt not found)\n",
            ),
        ),
        "/env" => Response::text(200, env_report(&|k| std::env::var(k).ok(), share)),
        _ => Response::text(404, format!("no route {path}\nsee /\n")),
    }
}

/// `?bytes=N` for `/log/tail`: default 64 KiB, 400 on junk, 413 past the cap.
fn tail_bytes(query: &str) -> Result<usize, Response> {
    if query.is_empty() {
        return Ok(TAIL_DEFAULT);
    }
    let Some(value) = query.strip_prefix("bytes=") else {
        return Err(Response::text(
            400,
            "unsupported query; expected ?bytes=N\n".to_string(),
        ));
    };
    let Ok(n) = value.parse::<usize>() else {
        return Err(Response::text(
            400,
            "?bytes expects a decimal count\n".to_string(),
        ));
    };
    if n > TAIL_CAP {
        return Err(Response::text(
            413,
            format!("bytes exceeds the cap ({TAIL_CAP})\n"),
        ));
    }
    Ok(n)
}

/// The latest presented canvas as a BMP; an all-black 550x400 frame before
/// the first present (so the route always answers with a valid image).
fn frame_bmp(share: &Arc<DebugShare>) -> Vec<u8> {
    let frame = share.frame.lock().ok().and_then(|g| g.clone());
    match frame {
        Some(px) if px.len() == WIDTH * HEIGHT * 3 => bmp_rgb(WIDTH, HEIGHT, &px),
        _ => bmp_rgb(WIDTH, HEIGHT, &Canvas::new(WIDTH, HEIGHT).px),
    }
}

/// Render bytes as text (lossy — a log is text, but a mangled byte must not
/// kill the route), or the fallback note when the file is missing.
fn text_body(data: Option<Vec<u8>>, missing: &str) -> String {
    match data {
        Some(body) => String::from_utf8_lossy(&body).into_owned(),
        None => missing.to_string(),
    }
}

/// Whole file up to `cap`; past it, the LAST `cap` bytes with a leading
/// truncation marker (the newest evidence is what a log is for).
pub fn read_capped(path: &Path, cap: usize) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len() as usize;
    if len <= cap {
        let mut body = Vec::with_capacity(len);
        file.read_to_end(&mut body).ok()?;
        return Some(body);
    }
    file.seek(SeekFrom::Start((len - cap) as u64)).ok()?;
    let mut body = format!("# truncated: showing the last {cap} of {len} bytes\n").into_bytes();
    body.reserve(cap);
    let mut rest = Vec::with_capacity(cap);
    file.read_to_end(&mut rest).ok()?;
    body.extend_from_slice(&rest);
    Some(body)
}

/// The last `n` bytes of a file (all of it when shorter). Bounded by `n`.
pub fn read_tail(path: &Path, n: usize) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len() as usize;
    let start = len.saturating_sub(n);
    file.seek(SeekFrom::Start(start as u64)).ok()?;
    let mut body = Vec::with_capacity(len - start);
    file.read_to_end(&mut body).ok()?;
    Some(body)
}

/// The first readable candidate, capped (the cookie jar's fallback rule).
fn read_first(paths: &[PathBuf], cap: usize) -> Option<Vec<u8>> {
    paths.iter().find_map(|p| read_capped(p, cap))
}

/// 24-bit BMP, bottom-up BGR rows with 4-byte padding — byte-for-byte the
/// layout `Canvas::save_bmp` writes (a unit test pins that, so the two can
/// never drift). Here so a route can serve a BMP without touching disk.
pub fn bmp_rgb(w: usize, h: usize, px: &[u8]) -> Vec<u8> {
    let row = (w * 3).div_ceil(4) * 4;
    let data = row * h;
    let mut out = Vec::with_capacity(54 + data);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((54 + data) as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(data as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    let pad = row - w * 3;
    for y in (0..h).rev() {
        for x in 0..w {
            let at = (y * w + x) * 3;
            let p = px.get(at..at + 3).unwrap_or(&[0u8, 0, 0]);
            out.extend_from_slice(&[p[2], p[1], p[0]]);
        }
        out.extend(std::iter::repeat_n(0u8, pad));
    }
    out
}

fn write_response(stream: &mut TcpStream, r: &Response) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        r.status,
        reason(r.status),
        r.content_type,
        r.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&r.body)?;
    stream.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "Internal Server Error",
    }
}

/// The `/env` body. `lookup` is the seam the test drives (the live server
/// passes `std::env::var`); only [`ENV_ALLOW`] is ever consulted.
fn env_report(lookup: &dyn Fn(&str) -> Option<String>, share: &Arc<DebugShare>) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(s, "NFM debug environment — explicit allow-list, read-only");
    let _ = writeln!(s, "profile: debug (this route exists only in debug builds)");
    let _ = writeln!(
        s,
        "version: {} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    let _ = writeln!(
        s,
        "host: {}/{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let audio = share.audio.lock().map(|g| g.clone()).unwrap_or_default();
    let _ = writeln!(
        s,
        "audio: {}",
        if audio.is_empty() {
            "(not opened yet)"
        } else {
            &audio
        }
    );
    let pads = share.pads.lock().map(|g| g.clone()).unwrap_or_default();
    let _ = writeln!(s, "pads: {}", pads.len());
    for p in &pads {
        let _ = writeln!(s, "  {p}");
    }
    let _ = writeln!(s, "env:");
    for name in ENV_ALLOW {
        match lookup(name) {
            Some(value) => {
                let _ = writeln!(s, "  {name}={}", clip_value(&value));
            }
            None => {
                let _ = writeln!(s, "  {name}=(unset)");
            }
        }
    }
    let _ = writeln!(
        s,
        "note: XAUTHORITY is reported as a path only; its contents are never read"
    );
    s
}

/// Cap an env value and make control bytes visible as `?` (one line per var).
fn clip_value(value: &str) -> String {
    value
        .chars()
        .take(VALUE_CAP)
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

const INDEX: &str = concat!(
    "<!doctype html><html><head><meta charset=\"utf-8\"><title>NFM debug</title></head><body>",
    "<h1>Need for Madness — debug server</h1>",
    "<p><b>DEBUG BUILDS ONLY.</b> Read-only, LAN-exposed by design; nothing is ever written.</p>",
    "<table border=1 cellpadding=4><tr><th>route</th><th>what</th></tr>",
    "<tr><td><a href=\"/log\">/log</a></td><td>nfm.log (whole; last 4 MiB if larger)</td></tr>",
    "<tr><td><a href=\"/log/tail\">/log/tail</a></td><td>nfm.log tail (64 KiB default, ?bytes=N, cap 4 MiB)</td></tr>",
    "<tr><td><a href=\"/probe.log\">/probe.log</a></td><td>nfm-probe.log, every pad event (cap 4 MiB)</td></tr>",
    "<tr><td><a href=\"/frame.bmp\">/frame.bmp</a></td><td>the latest presented 550x400 frame</td></tr>",
    "<tr><td><a href=\"/keys\">/keys</a></td><td>the WATCH table state + recent key/pad edges</td></tr>",
    "<tr><td><a href=\"/cookies\">/cookies</a></td><td>nfm-cookies.txt (cap 64 KiB)</td></tr>",
    "<tr><td><a href=\"/env\">/env</a></td><td>the display/env allow-list + build/audio/pad facts</td></tr>",
    "</table></body></html>\n"
);

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nfm-debugserve-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn empty_share() -> Arc<DebugShare> {
        Arc::new(DebugShare::default())
    }

    fn ctx(dir: &Path) -> (Arc<DebugShare>, Paths) {
        let paths = Paths {
            log: dir.join("nfm.log"),
            probe: dir.join("nfm-probe.log"),
            cookies: vec![dir.join("nfm-cookies.txt")],
        };
        (empty_share(), paths)
    }

    /// The BMP encoder is byte-identical to the port's headless writer, so
    /// `/frame.bmp` cannot drift from `cargo run -- stage ...` output.
    #[test]
    fn bmp_bytes_match_the_headless_writer() {
        let dir = scratch("bmp-eq");
        let (mut cv, w, h) = (Canvas::new(9, 4), 9, 4);
        for (i, px) in cv.px.chunks_exact_mut(3).enumerate() {
            px[0] = (i * 3) as u8;
            px[1] = (i * 5) as u8;
            px[2] = (i * 7) as u8;
        }
        let path = dir.join("ref.bmp");
        cv.save_bmp(path.to_str().unwrap()).unwrap();
        let reference = std::fs::read(&path).unwrap();
        assert_eq!(bmp_rgb(w, h, &cv.px), reference);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The served frame carries the applet's 550x400, 24 bpp header.
    #[test]
    fn bmp_header_is_550x400_24bpp() {
        let bmp = bmp_rgb(WIDTH, HEIGHT, &Canvas::new(WIDTH, HEIGHT).px);
        assert_eq!(&bmp[0..2], b"BM");
        let size = u32::from_le_bytes(bmp[2..6].try_into().unwrap());
        assert_eq!(size as usize, bmp.len());
        assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 550);
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), 400);
        assert_eq!(u16::from_le_bytes(bmp[26..28].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 24);
        assert_eq!(
            u32::from_le_bytes(bmp[34..38].try_into().unwrap()) as usize,
            ((550 * 3) as usize).div_ceil(4) * 4 * 400
        );
    }

    #[test]
    fn request_line_parses_only_a_well_formed_head() {
        assert_eq!(
            parse_request(b"GET /env HTTP/1.1\r\nHost: x\r\n\r\n"),
            Some(("GET".to_string(), "/env".to_string()))
        );
        assert_eq!(
            parse_request(b"GET /log/tail?bytes=10 HTTP/1.1\r\n\r\n"),
            Some(("GET".to_string(), "/log/tail?bytes=10".to_string()))
        );
        assert_eq!(parse_request(b"garbage\r\n\r\n"), None);
        assert_eq!(parse_request(b"GET relative HTTP/1.1\r\n"), None);
        assert_eq!(parse_request(b""), None);
    }

    /// Tails are bounded to the requested window, and whole-file reads past
    /// the cap report the truncation and keep the newest bytes.
    #[test]
    fn file_reads_are_bounded_tails() {
        let dir = scratch("tails");
        let path = dir.join("data.txt");
        std::fs::write(&path, b"0123456789").unwrap();
        assert_eq!(read_tail(&path, 4).unwrap(), b"6789");
        assert_eq!(read_tail(&path, 99).unwrap(), b"0123456789");
        assert_eq!(read_capped(&path, 99).unwrap(), b"0123456789");
        let capped = read_capped(&path, 4).unwrap();
        let text = String::from_utf8(capped).unwrap();
        assert!(text.starts_with("# truncated: showing the last 4 of 10 bytes"));
        assert!(text.ends_with("6789"));
        assert_eq!(read_tail(&dir.join("nope"), 4), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The dispatcher: every route resolves, unknown paths are 404, non-GET
    /// is 405, and the tail query is validated (400 junk / 413 past cap).
    #[test]
    fn route_table_answers_and_rejects() {
        let dir = scratch("routes");
        std::fs::write(dir.join("nfm.log"), "line one\nline two\n").unwrap();
        std::fs::write(dir.join("nfm-probe.log"), "0 ms ButtonPressed South\n").unwrap();
        std::fs::write(dir.join("nfm-cookies.txt"), "stage=2\n").unwrap();
        let (share, paths) = ctx(&dir);
        if let Ok(mut keys) = share.keys.lock() {
            *keys = "WATCH\n  [x] Up 1004\n".to_string();
        }

        let index = route(&share, &paths, "GET", "/");
        assert_eq!(index.status, 200);
        assert!(index.content_type.starts_with("text/html"));
        assert!(String::from_utf8_lossy(&index.body).contains("/frame.bmp"));

        let log = route(&share, &paths, "GET", "/log");
        assert_eq!(log.status, 200);
        assert!(String::from_utf8_lossy(&log.body).contains("line one"));

        let tail = route(&share, &paths, "GET", "/log/tail?bytes=9");
        assert_eq!(String::from_utf8_lossy(&tail.body), "line two\n");

        let probe = route(&share, &paths, "GET", "/probe.log");
        assert!(String::from_utf8_lossy(&probe.body).contains("South"));

        let keys = route(&share, &paths, "GET", "/keys");
        assert!(String::from_utf8_lossy(&keys.body).contains("WATCH"));

        let cookies = route(&share, &paths, "GET", "/cookies");
        assert_eq!(String::from_utf8_lossy(&cookies.body), "stage=2\n");

        let env = route(&share, &paths, "GET", "/env");
        assert_eq!(env.status, 200);
        assert!(String::from_utf8_lossy(&env.body).contains("XAUTHORITY"));

        assert_eq!(route(&share, &paths, "GET", "/nope").status, 404);
        assert_eq!(route(&share, &paths, "POST", "/env").status, 405);
        assert_eq!(route(&share, &paths, "GET", "/log/tail?nope=1").status, 400);
        assert_eq!(
            route(&share, &paths, "GET", "/log/tail?bytes=abc").status,
            400
        );
        assert_eq!(
            route(&share, &paths, "GET", "/log/tail?bytes=99999999").status,
            413
        );
        // defaults still answer when the file is absent
        let (share, paths) = ctx(&dir.join("absent-dir"));
        assert_eq!(route(&share, &paths, "GET", "/log").status, 200);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `/frame.bmp` serves the shared canvas, and before the first present a
    /// valid all-black frame (never a 404).
    #[test]
    fn frame_route_serves_the_shared_canvas() {
        let dir = scratch("frame");
        let (share, paths) = ctx(&dir);
        let black = route(&share, &paths, "GET", "/frame.bmp");
        assert_eq!(black.status, 200);
        assert_eq!(black.content_type, "image/bmp");
        assert_eq!(&black.body[0..2], b"BM");

        let mut px = vec![0u8; WIDTH * HEIGHT * 3];
        px[0] = 200; // top-left red byte, which lands in the BMP's last row
        if let Ok(mut frame) = share.frame.lock() {
            *frame = Some(px);
        }
        let painted = route(&share, &paths, "GET", "/frame.bmp");
        let last_row = 54 + (HEIGHT - 1) * ((WIDTH * 3).div_ceil(4) * 4);
        assert_eq!(painted.body[last_row + 2], 200, "BGR: red in byte 2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `/env` consults ONLY the allow-list: a lookup that would answer for an
    /// arbitrary secret name is never asked for it.
    #[test]
    fn env_route_is_an_allow_list() {
        let share = empty_share();
        let lookup = |name: &str| match name {
            "WAYLAND_DISPLAY" => Some("wayland-0".to_string()),
            "XAUTHORITY" => Some("/run/user/1000/xauth".to_string()),
            "SOME_SECRET" => Some("hunter2".to_string()),
            _ => None,
        };
        let body = env_report(&lookup, &share);
        assert!(body.contains("WAYLAND_DISPLAY=wayland-0"));
        assert!(body.contains("XAUTHORITY=/run/user/1000/xauth"));
        assert!(body.contains("XDG_SESSION_TYPE=(unset)"));
        assert!(body.contains("profile: debug"));
        assert!(
            !body.contains("hunter2"),
            "unlisted variables are never read"
        );
        assert!(!body.contains("SOME_SECRET"));
    }

    /// Env values are clamped and made single-line.
    #[test]
    fn env_values_are_clipped() {
        let long = "a".repeat(VALUE_CAP + 50);
        assert_eq!(clip_value(&long).chars().count(), VALUE_CAP);
        assert_eq!(clip_value("way\nland"), "way?land");
    }

    /// The pads/audio extras reach `/env` from the play thread's snapshot.
    #[test]
    fn env_reports_the_published_pad_and_audio_facts() {
        let share = empty_share();
        if let Ok(mut p) = share.pads.lock() {
            *p = vec!["0: Microsoft X-Box 360 pad".to_string()];
        }
        if let Ok(mut a) = share.audio.lock() {
            *a = "MacBook Speakers — 48000 Hz, 2ch, F32".to_string();
        }
        let body = env_report(&|_| None, &share);
        assert!(body.contains("pads: 1"));
        assert!(body.contains("0: Microsoft X-Box 360 pad"));
        assert!(body.contains("MacBook Speakers"));
    }
}
