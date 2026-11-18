//! The xt screen layer — pixel transforms for the HUD/menu sprites, seeded
//! as free functions ahead of the `xtGraphics` state-machine port. Each
//! function mirrors one decompiled `xtGraphics` method line-for-line, on
//! the applet's own ARGB pixel representation (the `PixelGrabber` `int[]`
//! shape): `Gif` values from `gif::decode` are those images.
//!
//! Transparency model: decoded GIF pixels are alpha 0 (palette color kept)
//! or alpha 255, and the `MemoryImageSource` round-trip the transforms feed
//! preserves exactly what the code writes into the int array, so working
//! directly on the `u32`s is the identity-equivalent of that pipeline.
//!
//! The `XtGraphics` struct is the applet's HUD/race-screen state machine
//! (`stat`, `resetstat`, `sortcars`, `snap`, `arrow`, `drawstat`, `drawcs`).
//! The engine's shared objects (`Medium`/`Madness`/...) thread through as
//! parameters, mirroring the source's reference fields. Deferred slices:
//! every `AudioClip` call (`play`/`loop`/`stop`) in these methods is
//! omitted until the audio layer; the two JPEG archive entries decode
//! through the toolkit's JPEG path (`jpeg`); the loose-file loads
//! (`hello`/`sign`/`loadbar`, the applet dir's
//! loading screen) ride with `loadpak` (this slice). The `RadicalMod`
//! music voices (`stracks`/`stages`/`cars`, `radicalmod`) are wired —
//! only the wall-time `AudioPlayer` drain stays deferred (see
//! `radicalmod` docs). `fase`/`oldfase`/`shaded`
//! live on the struct now (the source fields; every source write routes
//! through `var6.fase`).
//!
//! Layout (the R3 milestone split — the public API is unchanged):
//!
//! - `mod.rs` — the `XtGraphics` struct, `new()`, the archive/sound spine
//!   (`loadpak`, `random`, `snap`/`resetstat`/`sortcars`), the private
//!   loader helpers, and the parent-private color clamps (`clampc`,
//!   `lift`, `sink`) that the draw/HUD slices share.
//! - `screens` — the screen state machines: loading, clicknow, rad, cantgo,
//!   loadingstage, stageselect, inst, credits, maini, musicomp, framer,
//!   loadmusic, trackbg, cantreply, replyn, nofocus, loadingfailed,
//!   over/overon, levelhigh, carselect, finish.
//! - `hud` — the race HUD (`stat`, `drawstat`, `arrow`, `ctachm`) and the
//!   pause/flex machines (`pauseimage`, `fleximage`, `pausedgame`).
//! - `draw` — the sprite pixel transforms (`loadsnap`/`loadopsnap`/
//!   `pressed`/`bressed`), `drawcs`, and the pure projection helpers
//!   (`xs`/`ys`/`py`/`rot`).
//! - `audio` — the audio-control slice (`playsounds`, `sparkeng`, `crash`,
//!   `skid`, `stopairs`, `stopallnow`, `bind_audio`).
//! - `testutil` — the `cfg(test)` rig the submodule test sections share.
//!
//! The engine's shared objects (`Medium`/`Madness`/...) still thread through
//! as parameters, mirroring the source's reference fields.

use std::sync::{Arc, Mutex};

use crate::audiobus::AudioBus;
use crate::audioclip::AudioClip;
use crate::canvas::Canvas;
use crate::checkpoints::CheckPoints;
use crate::color;
use crate::conto::Conto;
use crate::control::Control;
use crate::gif::{self, Gif};
use crate::jpeg;
use crate::madness::{ACELF, AIRC, AIRS, GRIP, MAXMAG, Madness, OUTDAM, POWERLOSS, SWITS};
use crate::medium::Medium;
use crate::radicalmod::RadicalMod;
use crate::record::Record;
use crate::trackers::Trackers;
use crate::zipo::Zipo;

mod audio;
mod draw;
mod hud;
mod screens;

#[cfg(test)]
mod testutil;

pub use draw::{GRAY, bressed, loadopsnap, loadsnap, pressed, py, rot, xs, ys};

fn clampc(c: [i32; 3]) -> [u8; 3] {
    [
        c[0].clamp(0, 255) as u8,
        c[1].clamp(0, 255) as u8,
        c[2].clamp(0, 255) as u8,
    ]
}

/// One snap-brighten channel: Java
/// `(int)((float)c + (float)c * ((float)snap / 100.0F))`, clamped 0..255 —
/// the arrow/drawstat/drawcs color lifts (the loadsnap variant clamps
/// 0..225 instead).
fn lift(c: i32, snap: i32) -> i32 {
    let v = (c as f32 + c as f32 * (snap as f32 / 100.0f32)) as i32;
    v.clamp(0, 255)
}

/// One snap-darken channel (`drawcs` mode 4).
fn sink(c: i32, snap: i32) -> i32 {
    let v = (c as f32 - c as f32 * (snap as f32 / 100.0f32)) as i32;
    v.clamp(0, 255)
}

/// `xtGraphics` (xt:14-196): the HUD/race-screen state machine. The shared
/// engine objects (`Medium`, `Madness`, ...) stay with the callers and
/// thread through the methods as parameters, mirroring the source's
/// reference fields. Unfetched sprites stay `Gif::empty()` (the source's
/// `null`, which would NPE where this draws nothing).
pub struct XtGraphics {
    // The o* raw archive decodes; the plain names are the snap-prepared or
    // pressed/bressed derivatives the draw paths read.
    pub odmg: Gif,
    pub opwr: Gif,
    pub opos: Gif,
    pub owas: Gif,
    pub olap: Gif,
    pub oyourwasted: Gif,
    pub oyoulost: Gif,
    pub oyouwon: Gif,
    pub oyouwastedem: Gif,
    pub ogameh: Gif,
    pub oloadingmusic: Gif,
    pub omdness: Gif,
    pub ostar: [Gif; 2],
    pub orank: [Gif; 5],
    pub ocntdn: [Gif; 4],
    pub dmg: Gif,
    pub pwr: Gif,
    pub pos: Gif,
    pub was: Gif,
    pub lap: Gif,
    pub yourwasted: Gif,
    pub youlost: Gif,
    pub youwon: Gif,
    pub youwastedem: Gif,
    pub gameh: Gif,
    pub loadingmusic: Gif,
    pub mdness: Gif,
    pub star: [Gif; 3],
    pub rank: [Gif; 5],
    pub cntdn: [Gif; 4],
    // The menu frame and buttons; the variant indexes derive in loadpak's
    // tail like the constructor's.
    pub next: [Gif; 3],
    pub back: [Gif; 3],
    pub contin1: [Gif; 2],
    pub contin2: [Gif; 2],
    pub bl: Gif,
    pub bt: Gif,
    pub br: Gif,
    pub bb: Gif,
    pub select: Gif,
    pub paused: Gif,
    pub maini: Gif,
    pub opti: Gif,
    pub inst1: Gif,
    pub inst2: Gif,
    pub inst3: Gif,
    pub pgate: Gif,
    pub selectcar: Gif,
    pub statb: Gif,
    pub congrd: Gif,
    pub gameov: Gif,
    pub carsbg: Gif,
    pub radicalplay: Gif,
    pub rpro: Gif,
    pub nfmcom: Gif,
    /// `bgmain.jpg` (images2) → `bgmain`, `stages.jpg` (images3) →
    /// `trackbg` — the two JPEG entries, decoded through the toolkit's
    /// JPEG path (see `jpeg`).
    pub bgmain: Gif,
    pub trackbg: Gif,
    /// `flexpix` — the `pauseimage` capture canvas, 550×400 ints.
    pub flexpix: Vec<u32>,
    /// `Image fleximg` (xt:44) — the fade frame `pauseimage` builds from
    /// `flexpix` (`new MemoryImageSource(550, 400, flexpix, 0, 550)`).
    /// Stays an empty gif until the flex/pause slice ports `pauseimage`;
    /// the one branch that draws it today (`inst` returning to the in-race
    /// pause, fase -7) no-ops on an empty gif exactly like a null image
    /// would.
    pub fleximg: Gif,

    // --- selection and screen state (xt:25-196) ---
    /// `int fase = 111` — the shell's screen id; the applet boot lands on
    /// the click-now screen (fase 111 → 9 → 10 → menu). Param-threaded by
    /// the port until this slice; every source write in the game writes
    /// `var6.fase`, so the field lives here now.
    pub fase: i32,
    pub oldfase: i32,
    /// `boolean shaded` — the paused overlay flag; the menu paints key
    /// their shading pass on it (xt:1363-1406, 3186-3228) and the
    /// mouse-touch arm sets it (xt:3816).
    pub shaded: bool,
    pub sc: [i32; 5],
    pub xstart: [i32; 5],
    pub zstart: [i32; 5],
    pub proba: [f32; 10],
    pub dishandle: [f32; 10],
    pub unlocked: i32,
    pub opselect: i32,
    pub flipo: i32,
    pub nextc: bool,
    pub gatey: i32,
    pub looped: i32,
    pub pcontin: i32,
    pub pnext: i32,
    pub pback: i32,
    pub pstar: i32,
    pub lockcnt: i32,
    pub gocnt: i32,
    pub starcnt: i32,
    pub mutes: bool,
    /// `boolean[] loadedt = new boolean[11]` — the per-track
    /// music-armed flags `loadmusic` sets (only when the mix succeeded:
    /// the source's `if (this.stracks[i].stream != null)`).
    pub loadedt: [bool; 11],
    /// `RadicalMod[] stracks = new RadicalMod[11]` — one looping voice
    /// per stage, built by `loadmusic`. `None` before its stage loads.
    pub stracks: [Option<RadicalMod>; 11],
    /// `RadicalMod stages` — the track-select jingle, built by `loadpak`'s
    /// tail. `None` until then.
    pub stages: Option<RadicalMod>,
    /// `RadicalMod cars` — the car-select jingle, built by `loadpak`'s
    /// tail. `None` until then.
    pub cars: Option<RadicalMod>,
    /// `AudioClip[][] engs = new AudioClip[2][5]` — the engine voices:
    /// row 0 the `aN` set, row 1 the `bN` set (xt:106). `None` until
    /// `loadpak`'s sound tail fills them.
    pub engs: [[Option<AudioClip>; 5]; 2],
    /// `boolean[] pengs = new boolean[5]` — which engine voice currently
    /// loops (xt:107); `sparkeng` arbitrates.
    pub pengs: [bool; 5],
    /// `int[][] enginsignature` (xt:108-119) — per-car engine voice map.
    pub enginsignature: [[i32; 5]; 10],
    /// `AudioClip[] air = new AudioClip[6]` — the airborne rattles.
    pub air: [Option<AudioClip>; 6],
    /// `AudioClip[] crash/lowcrash = new AudioClip[3]` — the hard/soft
    /// crash banks; `crshturn` round-robins the soft/mid band.
    pub crash: [Option<AudioClip>; 3],
    pub lowcrash: [Option<AudioClip>; 3],
    /// Single-shot voices (xt:125-134): tires, checkpoint, carfixed,
    /// powerup, the three/two/one/go countdown, wasted, firewasted.
    pub tires: Option<AudioClip>,
    pub checkpoint: Option<AudioClip>,
    pub carfixed: Option<AudioClip>,
    pub powerup: Option<AudioClip>,
    pub three: Option<AudioClip>,
    pub two: Option<AudioClip>,
    pub one: Option<AudioClip>,
    pub go: Option<AudioClip>,
    pub wastd: Option<AudioClip>,
    pub firewasted: Option<AudioClip>,
    /// `AudioClip[] skid/dustskid = new AudioClip[2]` — skid alternates
    /// via `skflg`, dust-skid via `dskflg`.
    pub skid: [Option<AudioClip>; 2],
    pub dustskid: [Option<AudioClip>; 2],
    /// `boolean pwastd` — the wasted loop's arm flag.
    pub pwastd: bool,
    pub mutem: bool,
    pub arrace: bool,
    pub ana: i32,
    pub cntan: i32,
    pub cntovn: i32,
    pub flk: bool,
    pub tcnt: i32,
    pub tflk: bool,
    pub say: String,
    pub wasay: bool,
    pub clear: i32,
    pub posit: i32,
    pub wasted: i32,
    pub laps: i32,
    pub dested: [i32; 5],
    pub names: [&'static str; 10],
    pub dmcnt: i32,
    pub dmflk: bool,
    pub pwcnt: i32,
    pub pwflk: bool,
    pub adj: [[&'static str; 3]; 5],
    pub exlm: [&'static str; 3],
    /// The source field is named `loop` (a Rust keyword); `loop_` follows
    /// the Madness field's precedent.
    pub loop_: String,
    pub spin: String,
    pub asay: String,
    pub auscnt: i32,
    pub aflk: bool,
    pub hipno: [i32; 11],
    /// `int[] sndsize` — the per-track archive sizes the music loader's
    /// "N KB" line quotes (`loadmusic`).
    pub sndsize: [i32; 11],
    /// The three loose codebase gifs, constructor-loaded via
    /// `getImage(getCodeBase(), ...)` (xt:2935-2951): the scripted
    /// intro image (550×217 GIF87a), the radicalplay logo (75×66) and
    /// the loading bar (238×21). Riding `loadpak` here — the port's ctor
    /// image spine.
    pub hello: Gif,
    pub sign: Gif,
    pub loadbar: Gif,
    /// `int kbload` / `int dnload` — total and downloaded KB; the loading
    /// screen's bar width and percentage run off the pair. The ctor
    /// seeds 416, switching to 514 when `sunytyp()` reports 2 — the
    /// Sun-plugin AppletContext path every deploy JDK takes, which is
    /// this port's standing variant.
    pub kbload: i32,
    pub dnload: i32,
    pub pin: i32,
    pub pwait: i32,
    pub stopcnt: i32,
    pub cntwis: i32,
    pub crshturn: i32,
    pub bfcrash: i32,
    pub bfskid: i32,
    pub crashup: bool,
    pub skflg: i32,
    pub dskflg: i32,
    pub grrd: bool,
    pub aird: bool,
    pub holdcnt: i32,
    pub winner: bool,
    /// Test-only asset-root override (the mirror batch's fixture seam): the
    /// applet resolves every fetch against its codebase; the port probes
    /// `assets/` then `../../assets/`. The Java pin sheet's fixture
    /// codebases (`xtsparse`, `xtno3`, `xtno4`, `gfxonly` — a directory of
    /// chosen files) have no filesystem equivalent otherwise, so tests
    /// point this at one root. Production is `None` (the two-root probe).
    #[cfg(test)]
    pub asset_root: Option<String>,
    /// Test-only record of every sound path `loadpak` fetched, in source
    /// order — the Java fixture's `App.sounds` list (the applet's
    /// `getAudioClip` seam); the ctor's `sounds/newsun/` pins read it.
    #[cfg(test)]
    pub sound_log: Vec<String>,
    /// Test-only record of every loader error `loadpak` printed (the Java
    /// class captures stdout for the same pins; Rust tests cannot). One
    /// entry per printed line, verbatim.
    #[cfg(test)]
    pub load_errors: Vec<String>,
}

impl Default for XtGraphics {
    fn default() -> Self {
        Self::new()
    }
}

impl XtGraphics {
    pub fn new() -> XtGraphics {
        use crate::stage::{XSTART, ZSTART};
        XtGraphics {
            odmg: Gif::empty(),
            opwr: Gif::empty(),
            opos: Gif::empty(),
            owas: Gif::empty(),
            olap: Gif::empty(),
            oyourwasted: Gif::empty(),
            oyoulost: Gif::empty(),
            oyouwon: Gif::empty(),
            oyouwastedem: Gif::empty(),
            ogameh: Gif::empty(),
            oloadingmusic: Gif::empty(),
            omdness: Gif::empty(),
            ostar: [Gif::empty(), Gif::empty()],
            orank: [
                Gif::empty(),
                Gif::empty(),
                Gif::empty(),
                Gif::empty(),
                Gif::empty(),
            ],
            ocntdn: [Gif::empty(), Gif::empty(), Gif::empty(), Gif::empty()],
            dmg: Gif::empty(),
            pwr: Gif::empty(),
            pos: Gif::empty(),
            was: Gif::empty(),
            lap: Gif::empty(),
            yourwasted: Gif::empty(),
            youlost: Gif::empty(),
            youwon: Gif::empty(),
            youwastedem: Gif::empty(),
            gameh: Gif::empty(),
            loadingmusic: Gif::empty(),
            mdness: Gif::empty(),
            star: [Gif::empty(), Gif::empty(), Gif::empty()],
            rank: [
                Gif::empty(),
                Gif::empty(),
                Gif::empty(),
                Gif::empty(),
                Gif::empty(),
            ],
            cntdn: [Gif::empty(), Gif::empty(), Gif::empty(), Gif::empty()],
            next: [Gif::empty(), Gif::empty(), Gif::empty()],
            back: [Gif::empty(), Gif::empty(), Gif::empty()],
            contin1: [Gif::empty(), Gif::empty()],
            contin2: [Gif::empty(), Gif::empty()],
            bl: Gif::empty(),
            bt: Gif::empty(),
            br: Gif::empty(),
            bb: Gif::empty(),
            select: Gif::empty(),
            paused: Gif::empty(),
            maini: Gif::empty(),
            opti: Gif::empty(),
            inst1: Gif::empty(),
            inst2: Gif::empty(),
            inst3: Gif::empty(),
            pgate: Gif::empty(),
            selectcar: Gif::empty(),
            statb: Gif::empty(),
            congrd: Gif::empty(),
            gameov: Gif::empty(),
            carsbg: Gif::empty(),
            radicalplay: Gif::empty(),
            rpro: Gif::empty(),
            nfmcom: Gif::empty(),
            bgmain: Gif::empty(),
            trackbg: Gif::empty(),
            flexpix: vec![0; 550 * 400],
            fleximg: Gif::empty(),
            fase: 111,
            oldfase: 0,
            shaded: false,
            sc: [0; 5],
            xstart: XSTART,
            zstart: ZSTART,
            proba: [0.3, 0.8, 0.5, 0.3, 0.8, 0.0, 0.2, 0.4, 0.0, 0.0],
            dishandle: [0.65, 0.6, 0.55, 0.77, 0.7, 0.9, 0.7, 0.4, 1.0, 0.85],
            unlocked: 1,
            opselect: 1,
            flipo: 0,
            nextc: false,
            gatey: 0,
            looped: 1,
            pcontin: 0,
            pnext: 0,
            pback: 0,
            pstar: 0,
            lockcnt: 0,
            gocnt: 0,
            starcnt: 0,
            mutes: false,
            loadedt: [false; 11],
            stracks: [
                None, None, None, None, None, None, None, None, None, None, None,
            ],
            stages: None,
            cars: None,
            engs: [
                [None, None, None, None, None],
                [None, None, None, None, None],
            ],
            pengs: [false; 5],
            enginsignature: [
                [0, 0, 1, 1, 0],
                [0, 1, 1, 0, 1],
                [0, 0, 0, 0, 0],
                [0, 1, 1, 1, 1],
                [0, 0, 0, 1, 0],
                [0, 1, 1, 1, 1],
                [0, 1, 0, 1, 0],
                [1, 0, 0, 0, 0],
                [0, 1, 1, 1, 1],
                [1, 1, 1, 1, 1],
            ],
            air: [None, None, None, None, None, None],
            crash: [None, None, None],
            lowcrash: [None, None, None],
            tires: None,
            checkpoint: None,
            carfixed: None,
            powerup: None,
            three: None,
            two: None,
            one: None,
            go: None,
            wastd: None,
            firewasted: None,
            skid: [None, None],
            dustskid: [None, None],
            pwastd: false,
            mutem: false,
            arrace: false,
            ana: 0,
            cntan: 0,
            cntovn: 0,
            flk: false,
            tcnt: 30,
            tflk: false,
            say: String::new(),
            wasay: false,
            clear: 0,
            posit: 0,
            wasted: 0,
            laps: 0,
            dested: [0; 5],
            names: [
                "Tornado Shark",
                "Formula 7",
                "Wow Caninaro",
                "La vite Crab",
                "Nimi",
                "MAX Revenge",
                "Lead Oxide",
                "EL KING",
                "Radical One",
                "DR Monstaa",
            ],
            dmcnt: 0,
            dmflk: false,
            pwcnt: 0,
            pwflk: false,
            adj: [
                ["Cool", "Alright", "Nice"],
                ["Wicked", "Amazing", "Super"],
                ["Awesome", "Ripping", "Radical"],
                [
                    "What the...?",
                    "Your a super star!!!!",
                    "Who are you again...?",
                ],
                ["surf style", "off the lip", "bounce back"],
            ],
            exlm: ["!", "!!", "!!!"],
            loop_: String::new(),
            spin: String::new(),
            asay: String::new(),
            auscnt: 45,
            aflk: false,
            hipno: [0, 2, 2, 2, 2, 0, 2, 50, 2, 2, 6],
            sndsize: [39, 128, 23, 58, 106, 140, 81, 135, 38, 141, 80],
            hello: Gif::empty(),
            sign: Gif::empty(),
            loadbar: Gif::empty(),
            kbload: 514,
            dnload: 0,
            pin: 60,
            pwait: 7,
            stopcnt: 0,
            cntwis: 0,
            crshturn: 0,
            bfcrash: 0,
            bfskid: 0,
            crashup: false,
            skflg: 0,
            dskflg: 0,
            grrd: false,
            aird: false,
            holdcnt: 0,
            winner: false,
            #[cfg(test)]
            asset_root: None,
            #[cfg(test)]
            sound_log: Vec::new(),
            #[cfg(test)]
            load_errors: Vec::new(),
        }
    }

    /// One asset fetch through the codebase probe (see `asset_root`), in the
    /// build-mode order of the D2a pack contract: debug builds keep the
    /// historic `assets` → `../../assets` filesystem probe and fall back to
    /// the embedded pack (`assets/{tail}`); release builds read the pack
    /// only (no resource file is touched on disk). Without `tools/pack.sh`
    /// the embedded archive is empty, so the debug fallback is a no-op.
    fn asset(&self, tail: &str) -> Option<Vec<u8>> {
        #[cfg(test)]
        if let Some(root) = &self.asset_root {
            return std::fs::read(format!("{root}/{tail}")).ok();
        }
        #[cfg(debug_assertions)]
        {
            for base in ["assets", "../../assets"] {
                if let Ok(bytes) = std::fs::read(format!("{base}/{tail}")) {
                    return Some(bytes);
                }
            }
            crate::pack::get(&format!("assets/{tail}")).map(<[u8]>::to_vec)
        }
        #[cfg(not(debug_assertions))]
        {
            crate::pack::get(&format!("assets/{tail}")).map(<[u8]>::to_vec)
        }
    }

    /// Print a loader failure and keep it for the test-side stdout pins.
    fn load_error(&mut self, msg: String) {
        println!("{msg}");
        #[cfg(test)]
        self.load_errors.push(msg);
    }

    /// One `.au` fetch + parse (Java's `Applet.getAudioClip`): the path is
    /// logged for the ctor's sound-dir pins, `None` for a missing file.
    fn load_sound(&mut self, tail: &str) -> Option<AudioClip> {
        #[cfg(test)]
        self.sound_log.push(tail.to_string());
        self.asset(tail).and_then(|raw| AudioClip::parse(&raw).ok())
    }

    /// One music `.zipo`'s first entry (the module image); `None` when the
    /// archive is missing or unparsable.
    fn load_track(&self, tail: &str) -> Option<Vec<u8>> {
        self.asset(tail)
            .and_then(|raw| Zipo::read(&raw).ok())
            .and_then(|z| z.entries.into_iter().next().map(|e| e.data))
    }

    /// `Math.random()`: a fresh `[0,1)` double. Java's `xtGraphics.random()`
    /// has no state of its own — every call is a direct read of the JDK's
    /// single global holder — so this is the port's global slot
    /// ([`crate::random::math_random`]), never a per-instance generator.
    pub(crate) fn random(&self) -> f64 {
        crate::random::math_random()
    }

    /// The constructor's image spine (xt:2905-2987): first the three
    /// loose codebase gifs (`hello`/`sign`/`loadbar`, each behind its own
    /// MediaTracker wait in the source), then the four graphics archives
    /// and every pinned field, then the pressed/bressed button states.
    /// The source's `loading()` calls between the pak loads ride with the
    /// applet shell; `dnload` accrues there. Archive read or decode
    /// failures print the source's message and continue (the catch),
    /// leaving the field empty.
    ///
    /// Both halves resolve against the applet's codebase / document base;
    /// the port stands in for those with a workspace-root-or-crate-
    /// relative probe, so the same call works from the render CLI and
    /// from the test harness.
    pub fn loadpak(&mut self) {
        // xt:2935-2951 — the loose gifs load first, off the codebase root
        // (not from an archive): `getImage(getCodeBase(), "hello.gif")` —
        // the jar carries copies but the applet never reads them.
        for tail in [
            "graphics/hello.gif",
            "graphics/sign.gif",
            "graphics/loadbar.gif",
        ] {
            let raw = match self.asset(tail) {
                Some(raw) => raw,
                None => {
                    self.load_error(format!("Error Loading {tail}: no file found"));
                    continue;
                }
            };
            let g = match gif::decode(&raw) {
                Ok(g) => g,
                Err(e) => {
                    self.load_error(format!("Error Loading {tail}: {e}"));
                    continue;
                }
            };
            match tail {
                "graphics/hello.gif" => self.hello = g,
                "graphics/sign.gif" => self.sign = g,
                "graphics/loadbar.gif" => self.loadbar = g,
                _ => {}
            }
        }
        // The ctor's kbload line sits right after the gif waits
        // (xt:2953-2957): 416 KB total, 514 on the newsun build — the
        // deploy case (sunytyp() == 2 through the Sun plugin's
        // AppletContext; `new()` seeds it).
        for (pak, tail) in [
            (1, "graphics/images1.zipo"),
            (2, "graphics/images2.zipo"),
            (3, "graphics/images3.zipo"),
            (4, "graphics/images4.zipo"),
        ] {
            // xt:2981-2988 — the per-pak progress arithmetic is
            // unconditional (the source adds it after each loadpak call,
            // whose failures are swallowed).
            self.dnload += if pak == 2 || pak == 4 { 44 } else { 47 };
            let raw = match self.asset(tail) {
                Some(raw) => raw,
                None => {
                    self.load_error(format!("Error Loading Images Pak {pak}: no archive found"));
                    continue;
                }
            };
            let zipo = match Zipo::read(&raw) {
                Ok(z) => z,
                Err(e) => {
                    self.load_error(format!("Error Loading Images Pak {pak}: {e}"));
                    continue;
                }
            };
            for entry in &zipo.entries {
                // Toolkit.createImage sniffs the stream magic: SOI bytes
                // ride its JPEG decoder, everything else the native GIF
                // decoder. Bad bytes are taken silently (a broken Image
                // draws nothing) — stay silent too.
                let g = if entry.data.len() >= 2 && entry.data[0] == 0xFF && entry.data[1] == 0xD8 {
                    match jpeg::decode(&entry.data) {
                        Ok(g) => g,
                        Err(_) => continue,
                    }
                } else {
                    match gif::decode(&entry.data) {
                        Ok(g) => g,
                        Err(_) => continue,
                    }
                };
                match entry.name.as_str() {
                    "cars.gif" => self.carsbg = g,
                    "1.gif" => self.orank[0] = g,
                    "gameh.gif" => self.ogameh = g,
                    "gameov.gif" => self.gameov = g,
                    "lap.gif" => self.olap = g,
                    "paused.gif" => self.paused = g,
                    "select.gif" => self.select = g,
                    "yourwasted.gif" => self.oyourwasted = g,
                    "youwastedem.gif" => self.oyouwastedem = g,
                    "1c.gif" => self.ocntdn[1] = g,
                    "2c.gif" => self.ocntdn[2] = g,
                    "3c.gif" => self.ocntdn[3] = g,
                    "2.gif" => self.orank[1] = g,
                    "3.gif" => self.orank[2] = g,
                    "4.gif" => self.orank[3] = g,
                    "5.gif" => self.orank[4] = g,
                    "bgmain.jpg" => self.bgmain = g,
                    "stages.jpg" => self.trackbg = g,
                    "br.gif" => self.br = g,
                    "loadingmusic.gif" => self.oloadingmusic = g,
                    "main.gif" => self.maini = g,
                    "radicalplay.gif" => self.radicalplay = g,
                    "back.gif" => self.back[0] = g,
                    "continue1.gif" => self.contin1[0] = g,
                    "continue2.gif" => self.contin2[0] = g,
                    "inst3.gif" => self.inst3 = g,
                    "next.gif" => self.next[0] = g,
                    "pgate.gif" => self.pgate = g,
                    "rpro.gif" => self.rpro = g,
                    "selectcar.gif" => self.selectcar = g,
                    "youlost.gif" => self.oyoulost = g,
                    "youwon.gif" => self.oyouwon = g,
                    "0c.gif" => self.ocntdn[0] = g,
                    "damage.gif" => self.odmg = g,
                    "power.gif" => self.opwr = g,
                    "position.gif" => self.opos = g,
                    "wasted.gif" => self.owas = g,
                    "bl.gif" => self.bl = g,
                    "bt.gif" => self.bt = g,
                    "bb.gif" => self.bb = g,
                    "start1.gif" => self.ostar[0] = g,
                    "start2.gif" => self.ostar[1] = g,
                    "congrad.gif" => self.congrd = g,
                    "statb.gif" => self.statb = g,
                    "madness.gif" => self.omdness = g,
                    "options.gif" => self.opti = g,
                    "inst1.gif" => self.inst1 = g,
                    "inst2.gif" => self.inst2 = g,
                    "nfmcom.gif" => self.nfmcom = g,
                    _ => {}
                }
            }
        }
        // The constructor tail (xt:2977-2987): the pressed/bressed button
        // variants. The contin1/contin2 lines run twice in the source (an
        // unrolled 2-iteration loop the decompiler flattened); the second
        // run is the identity.
        self.next[1] = pressed(&self.next[0]);
        self.back[1] = pressed(&self.back[0]);
        self.next[2] = bressed(&self.next[0]);
        self.back[2] = bressed(&self.back[0]);
        self.contin1[1] = pressed(&self.contin1[0]);
        self.contin2[1] = bressed(&self.contin2[0]);
        self.contin1[1] = pressed(&self.contin1[0]);
        self.contin2[1] = bressed(&self.contin2[0]);
        self.star[2] = pressed(&self.ostar[1]);
        // The constructor's sound tail (xt:2986-3145, misattributed to
        // loadpak2 by the decompiler — bytecode places it in the ctor,
        // where slot 4 is the sunytyp() value): the `newsun/` variant is
        // the standing one on the deploy Sun plugin (sunytyp() == 2). The
        // per-file `dnload` accrual and the interleaved `loading()` paints
        // (xt:2989-3155) are the progress arithmetic; the paints are the
        // caller's (a headless load draws nothing), the accrual is here so
        // the loading screen's 485/514 numbers come out.
        for i in 0..5 {
            self.engs[0][i] = self.load_sound(&format!("sounds/newsun/a{i}.au"));
            self.dnload += 2;
            self.engs[1][i] = self.load_sound(&format!("sounds/newsun/b{i}.au"));
            self.dnload += 3;
            self.pengs[i] = false;
        }
        for i in 0..6 {
            self.air[i] = self.load_sound(&format!("sounds/newsun/air{i}.au"));
            self.dnload += 2;
        }
        for i in 0..3 {
            self.crash[i] = self.load_sound(&format!("sounds/newsun/crash{}.au", i + 1));
            self.dnload += 12;
            self.lowcrash[i] = self.load_sound(&format!("sounds/newsun/lowcrash{}.au", i + 1));
            self.dnload += 8;
        }
        self.tires = self.load_sound("sounds/newsun/tires.au");
        self.dnload += 12;
        self.checkpoint = self.load_sound("sounds/newsun/checkpoint.au");
        self.dnload += 12;
        self.carfixed = self.load_sound("sounds/newsun/carfixed.au");
        self.dnload += 16;
        self.powerup = self.load_sound("sounds/newsun/powerup.au");
        self.dnload += 12;
        self.three = self.load_sound("sounds/newsun/three.au");
        self.dnload += 12;
        self.two = self.load_sound("sounds/newsun/two.au");
        self.dnload += 12;
        self.one = self.load_sound("sounds/newsun/one.au");
        self.dnload += 12;
        self.go = self.load_sound("sounds/newsun/go.au");
        self.dnload += 12;
        for i in 0..2 {
            self.skid[i] = self.load_sound(&format!("sounds/newsun/skid{}.au", i + 1));
            self.dnload += 9;
            self.dustskid[i] = self.load_sound(&format!("sounds/newsun/dustskid{}.au", i + 1));
            self.dnload += 11;
        }
        self.wastd = self.load_sound("sounds/newsun/wasted.au");
        self.dnload += 5;
        self.firewasted = self.load_sound("sounds/newsun/firewasted.au");
        self.dnload += 13;
        // xt:3147-3155 — the two jingles, fetched after the sound tail.
        // A missing/unreadable archive parks `None` (a deferred load plays
        // nothing); a parsed module always yields `Some` (a failed mix
        // degrades to a null stream inside `RadicalMod`).
        self.cars = self
            .load_track("music/cars.zipo")
            .map(|b| RadicalMod::new(&b, 500, 7900, 125));
        self.dnload += 26;
        self.stages = self
            .load_track("music/stages.zipo")
            .map(|b| RadicalMod::new(&b, 200, 9000, 145));
        self.dnload += 22;
        // xt:3155-3157 — the ctor's loadedt[0..9] re-clear.
        for loaded in self.loadedt.iter_mut().take(10) {
            *loaded = false;
        }
    }

    /// `xtGraphics.snap(int stage)` (xt:2540): re-tint the whole HUD sprite
    /// set with the stage palette. Runs after every `sky` line of the stage
    /// file (GameSparker.java:336-337), so the last sky line wins.
    pub fn snap(&mut self, stage: i32, m: &Medium) {
        self.dmg = loadsnap(&self.odmg, m);
        self.pwr = loadsnap(&self.opwr, m);
        self.was = loadsnap(&self.owas, m);
        self.lap = loadsnap(&self.olap, m);
        self.pos = loadsnap(&self.opos, m);
        for i in 0..5 {
            self.rank[i] = loadsnap(&self.orank[i], m);
        }
        for i in 0..4 {
            self.cntdn[i] = loadsnap(&self.ocntdn[i], m);
        }
        self.yourwasted = loadsnap(&self.oyourwasted, m);
        self.youlost = loadsnap(&self.oyoulost, m);
        self.youwon = loadsnap(&self.oyouwon, m);
        self.youwastedem = loadsnap(&self.oyouwastedem, m);
        self.gameh = loadsnap(&self.ogameh, m);
        self.mdness = loadsnap(&self.omdness, m);
        self.loadingmusic = loadopsnap(&self.oloadingmusic, stage, &self.hipno, m);
        self.star[0] = loadopsnap(&self.ostar[0], stage, &self.hipno, m);
        self.star[1] = loadopsnap(&self.ostar[1], stage, &self.hipno, m);
    }

    /// `xtGraphics.resetstat(int stage)` (xt:2617): the per-race HUD reset.
    pub fn resetstat(&mut self, stage: i32) {
        self.arrace = false;
        self.ana = 0;
        self.cntan = 0;
        self.cntovn = 0;
        self.tcnt = 30;
        self.wasay = false;
        self.clear = 0;
        self.dmcnt = 0;
        self.pwcnt = 0;
        self.auscnt = 45;
        self.pnext = 0;
        self.pback = 0;
        self.starcnt = 90;
        self.gocnt = 3;
        self.grrd = true;
        self.aird = true;
        self.bfcrash = 0;
        self.cntwis = 0;
        self.bfskid = 0;
        self.pwait = 7;
        self.holdcnt = 0;
        self.winner = false;
        self.wasted = 0;
        for d in self.dested.iter_mut() {
            *d = 0;
        }
        self.sortcars(stage);
    }

    /// `xtGraphics.sortcars(int stage)` (xt:2278-2382): deal the opponent
    /// car models into `sc[1..]`. `sc[0]` (the player's pick) is never
    /// touched. The randomness is the global `Math.random()` slot
    /// ([`crate::random::math_random`]), so a given build rolls a fixed
    /// roster; the original re-rolled every run.
    pub fn sortcars(&mut self, stage: i32) {
        if stage == 0 {
            return;
        }
        let mut done = [false; 5];
        if self.unlocked == stage && self.unlocked != 11 {
            self.sc[4] = 4 + (stage + 1) / 2;
            let mut var7 = 1usize;
            loop {
                done[var7] = false;
                while !done[var7] {
                    self.sc[var7] = (self.random() * (4 + (stage + 1) / 2) as f64) as i32;
                    done[var7] = true;
                    for (var10, s) in self.sc.iter().enumerate() {
                        if var7 != var10 && self.sc[var7] == *s {
                            done[var7] = false;
                        }
                    }
                    if self.random() < self.proba[self.sc[var7] as usize] as f64 {
                        done[var7] = false;
                    }
                    if self.sc[var7] == 1 && stage == 6 {
                        done[var7] = false;
                    }
                    if self.sc[var7] == 7 && (stage == 8 || stage == 9 || stage == 7) {
                        done[var7] = false;
                    }
                }
                var7 += 1;
                if var7 >= 4 {
                    break;
                }
            }
            // xt:2317-2355 — the final party stage guarantees the King or
            // Radical One in the field.
            if stage == 10 {
                if self.sc[0] != 7 {
                    let mut king = false;
                    for s in self.sc {
                        if s == 7 {
                            king = true;
                        }
                    }
                    // xt:2324-2330 — the coin picks WHICH slot gets the
                    // King (the else arm), it never fills both.
                    if !king {
                        if self.random() > self.random() {
                            self.sc[1] = 7;
                        } else {
                            self.sc[2] = 7;
                        }
                    }
                } else {
                    let mut radical = false;
                    for s in self.sc {
                        if s == 8 {
                            radical = true;
                        }
                    }
                    if !radical && self.random() > self.random() {
                        self.sc[1] = 8;
                    }
                    if !radical {
                        self.sc[2] = 8;
                    }
                }
            }
        } else {
            let mut var3 = 5usize;
            if self.sc[0] != 4 + (stage + 1) / 2 && stage != 11 {
                self.sc[4] = 4 + (stage + 1) / 2;
                var3 = 4;
            }
            for (var4, done4) in done.iter_mut().enumerate().take(var3).skip(1) {
                *done4 = false;
                while !*done4 {
                    let mut var5 = self.unlocked;
                    if var5 == 11 {
                        var5 = 10;
                    }
                    self.sc[var4] = (self.random() * (5 + (var5 + 1) / 2) as f64) as i32;
                    *done4 = true;
                    for (var6, s) in self.sc.iter().enumerate() {
                        if var4 != var6 && self.sc[var4] == *s {
                            *done4 = false;
                        }
                    }
                    if self.random() < self.proba[self.sc[var4] as usize] as f64 {
                        *done4 = false;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::md5;
    use crate::xtgraphics::testutil::{dump, hex, loaded_xt, stage1_m};

    #[test]
    fn loadpak_maps_every_archive_and_the_button_variants() {
        let xt = loaded_xt();
        // a spot check across the four archives
        assert!(xt.odmg.w > 0, "images4 damage.gif");
        assert!(xt.orank[0].w > 0, "images1 1.gif");
        assert!(xt.ocntdn[3].w > 0, "images2 3c.gif");
        assert!(xt.oyoulost.w > 0, "images3 youlost.gif");
        // the two JPEG entries decode through the toolkit path
        assert_eq!((xt.bgmain.w, xt.bgmain.h), (550, 400), "bgmain.jpg decoded");
        assert_eq!(
            (xt.trackbg.w, xt.trackbg.h),
            (550, 400),
            "stages.jpg decoded"
        );
        // the pressed variant: every pixel becomes the corner key
        // (unchanged) or opaque black — the silhouette shape
        assert_eq!(xt.next[1].w, xt.next[0].w, "same dims as the raw");
        let key = xt.next[0].px[xt.next[0].px.len() - 1];
        assert!(
            xt.next[1].px.iter().all(|&p| p == 0xFF00_0000 || p == key),
            "pressed = silhouette keyed by the corner"
        );
        assert!(
            xt.next[1].px.contains(&0xFF00_0000),
            "the arrow silhouette went black"
        );
        // bressed fills (206, 214, 255) the same way
        assert!(
            xt.next[2].px.iter().all(|&p| p == 0xFFCE_D6FF || p == key),
            "bressed = the pale fill keyed by the corner"
        );
        // star[2] presses the RAW ostar[1], not the opsnap'd star[1]
        let okey = xt.ostar[1].px[xt.ostar[1].px.len() - 1];
        assert!(
            xt.star[2].px.iter().all(|&p| p == 0xFF00_0000 || p == okey),
            "star[2] = pressed(ostar[1])"
        );
    }

    #[test]
    fn snap_re_tints_the_hud_with_the_stage_palette() {
        let mut xt = loaded_xt();
        let m = stage1_m();
        xt.snap(1, &m);
        // the gray HUD panels key their glass with the stage sky: every
        // opaque gray (0xFFC0C0C0) pixel of the raw decode becomes the
        // packed sky, keyed pixels stay keyed
        let packed_sky = color::rgb(190, 215, 255);
        let gray_in_raw = xt.odmg.px.iter().filter(|&&p| p == 0xFFC0_C0C0).count();
        assert!(gray_in_raw > 300, "the damage panel carries gray");
        assert_eq!(
            xt.dmg.px.iter().filter(|&&p| p == packed_sky).count(),
            gray_in_raw,
            "every gray pixel became the sky"
        );
        assert_eq!(xt.rank[0].w, xt.orank[0].w, "the rank set re-derived");
        // re-snapping the same palette is the identity (the o* raws drive)
        let first = xt.dmg.px.clone();
        xt.snap(1, &m);
        assert_eq!(xt.dmg.px, first);
        // the loading-music and star sprites took the opsnap path
        assert_eq!(xt.loadingmusic.w, xt.oloadingmusic.w);
        assert_eq!(xt.star[0].w, xt.ostar[0].w);
    }

    #[test]
    fn resetstat_resets_and_sortcars_deals_a_legal_roster() {
        let mut xt = XtGraphics::new();
        xt.resetstat(1);
        assert_eq!(xt.starcnt, 90);
        assert_eq!(xt.gocnt, 3);
        assert!(!xt.arrace);
        assert_eq!(xt.auscnt, 45);
        assert_eq!(xt.tcnt, 30);
        assert_eq!(xt.pwait, 7);
        assert!(!xt.winner);
        assert_eq!(xt.sc[0], 0, "the player's pick is untouched");
        // the unlocked==stage deal: slot 4 takes the stage's signature
        // car, slots 1..3 roll distinct models out of the deal range
        assert_eq!(xt.sc[4], 5, "4 + (1+1)/2");
        for a in 1..4 {
            for b in 1..4 {
                if a != b {
                    assert_ne!(xt.sc[a], xt.sc[b], "slots {a}/{b} collided");
                }
            }
            assert!(
                (0..5).contains(&xt.sc[a]),
                "slot {a} = {} outside the stage-1 deal",
                xt.sc[a]
            );
        }

        // the stage's own ban set applies only on the unlocked==stage deal
        xt.unlocked = 6;
        xt.resetstat(6);
        assert!(
            xt.sc[1..4].iter().all(|&s| s != 1),
            "Formula 7 banned on stage 6: {:?}",
            xt.sc
        );

        // the party stage fields the King when the player didn't pick it
        xt.unlocked = 10;
        xt.resetstat(10);
        assert!(
            xt.sc.contains(&7),
            "the King guaranteed on stage 10: {:?}",
            xt.sc
        );
        // and the player driving the King fields Radical One
        xt.sc[0] = 7;
        xt.sortcars(10);
        assert_eq!(xt.sc[2], 8, "Radical One guaranteed opposite the King");
    }

    /// The Java inits this slice owns: fase boots on the click-now screen
    /// with `oldfase`/`shaded` cleared, kbload seeds through the deploy
    /// variant (`sunytyp()` == 2 through the Sun plugin's AppletContext —
    /// 416 → 514), the sndsize table stays, and the loose gifs sit empty
    /// until `loadpak` runs.
    #[test]
    fn loading_slice_fields_start_like_the_java() {
        let xt = XtGraphics::new();
        assert_eq!(xt.fase, 111, "int fase = 111");
        assert_eq!(xt.oldfase, 0);
        assert!(!xt.shaded);
        assert_eq!(
            xt.kbload, 514,
            "the deploy variant seeds 514, not the 1.3-era 416"
        );
        assert_eq!(xt.dnload, 0);
        assert_eq!(
            xt.sndsize,
            [39, 128, 23, 58, 106, 140, 81, 135, 38, 141, 80]
        );
        assert!(xt.loadedt.iter().all(|&b| !b));
        assert_eq!(xt.hello.w, 0, "hello sits empty until loadpak");
        assert_eq!(xt.sign.w, 0);
        assert_eq!(xt.loadbar.w, 0);
    }

    /// The three loose codebase gifs decode byte-identically to the JDK
    /// 1.8.0_40 `Toolkit.createImage` + `PixelGrabber` chain (the scratch
    /// `JpegGt` dump md5s; hello is GIF87a — no transparency — the two
    /// others GIF89a).
    #[test]
    fn loose_gifs_decode_as_the_jdk() {
        let g =
            crate::gif::decode(include_bytes!("../../../../assets/graphics/hello.gif")).unwrap();
        assert_eq!((g.w, g.h), (550, 217), "hello: GIF87a geometry");
        assert_eq!(
            hex(&md5(&dump(g.w as i32, g.h as i32, &g.px))),
            "63b224bd714550cb281d304f14e7dc1c"
        );

        let g = crate::gif::decode(include_bytes!("../../../../assets/graphics/sign.gif")).unwrap();
        assert_eq!((g.w, g.h), (75, 66), "sign: GIF89a geometry");
        assert_eq!(
            hex(&md5(&dump(g.w as i32, g.h as i32, &g.px))),
            "626d2d434fd92ae592ade07ce3805fbc"
        );

        let g =
            crate::gif::decode(include_bytes!("../../../../assets/graphics/loadbar.gif")).unwrap();
        assert_eq!((g.w, g.h), (238, 21), "loadbar: GIF89a geometry");
        assert_eq!(
            hex(&md5(&dump(g.w as i32, g.h as i32, &g.px))),
            "1c0d2a9d82997e53c3e71ada31335aee"
        );
    }
}
