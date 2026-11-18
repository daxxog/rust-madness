//! Parses every `.rad` in `assets/graphics/models.zipo` with the ported
//! `ContO(byte[])` constructor (nfm-core) and prints what it extracted.
//!
//! Also validates the archive against the canonical 43-name `loadbase()`
//! list (GameSparker.java:169-211) so any asset drift shows up as an error.

use std::process::ExitCode;

use nfm_core::model::Model;
use nfm_core::zipo::Zipo;

/// Verbatim from `loadbase()` (reference/vineflower/GameSparker.java:169-211).
const LOADBASE: [&str; 43] = [
    "2000tornados",
    "formula7",
    "canyenaro",
    "lescrab",
    "nimi",
    "maxrevenge",
    "leadoxide",
    "king",
    "radicalone",
    "drmonster",
    "road",
    "froad",
    "twister2",
    "twister1",
    "turn",
    "offroad",
    "bumproad",
    "offturn",
    "nroad",
    "nturn",
    "roblend",
    "noblend",
    "rnblend",
    "roadend",
    "offroadend",
    "hpground",
    "ramp30",
    "cramp35",
    "dramp15",
    "dhilo15",
    "slide10",
    "takeoff",
    "sramp22",
    "offbump",
    "offramp",
    "sofframp",
    "halfpipe",
    "spikes",
    "rail",
    "thewall",
    "checkpoint",
    "fixpoint",
    "offcheckpoint",
];

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets/graphics/models.zipo".to_string());
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let zipo = match Zipo::read(&bytes) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("bad zipo archive: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Set parity against loadbase(): every member must be <name>.rad.
    let mut names: Vec<&str> = zipo
        .entries
        .iter()
        .map(|e| e.name.strip_suffix(".rad").unwrap_or(&e.name))
        .collect();
    names.sort_unstable();
    let mut expected: Vec<&str> = LOADBASE.to_vec();
    expected.sort_unstable();
    if names != expected {
        eprintln!("archive names differ from loadbase():");
        for n in names.iter().chain(expected.iter()) {
            let (a, b) = (names.binary_search(n), expected.binary_search(n));
            match (a, b) {
                (Ok(_), Ok(_)) => {}
                (Ok(_), Err(_)) => eprintln!("  not in loadbase(): {n}"),
                (Err(_), Ok(_)) => eprintln!("  missing from archive: {n}"),
                (Err(_), Err(_)) => {}
            }
        }
        return ExitCode::FAILURE;
    }

    println!(
        "{:<14} {:>6} {:>6} {:>8} {:>5} {:>4} {:>7} {:>6} {:>7}  flags",
        "name", "planes", "wheels", "trackers", "maxR", "disp", "disline", "ground", "npl"
    );

    let mut failures = 0;
    for entry in &zipo.entries {
        match Model::parse_rad(&entry.name, &entry.data) {
            Ok(m) => {
                let mut flags = String::new();
                if m.shadow {
                    flags.push('s');
                }
                if m.noline {
                    flags.push('n');
                }
                if m.planes.iter().any(|p| p.glass) {
                    flags.push('g');
                }
                if m.planes.iter().any(|p| p.road) {
                    flags.push('r');
                }
                if flags.is_empty() {
                    flags.push('-');
                }
                println!(
                    "{:<14} {:>6} {:>6} {:>8} {:>5} {:>4} {:>7} {:>6.2} {:>7}  {}",
                    entry.name,
                    m.planes.len(),
                    m.wheels.len(),
                    m.trackers.len(),
                    m.max_r,
                    m.disp,
                    m.disline,
                    m.grounded,
                    m.npl_estimate(),
                    flags
                );
                if m.npl_estimate() > nfm_core::model::PLANE_CAP {
                    eprintln!(
                        " warning: {} npl {} exceeds original Plane[{}] cap",
                        entry.name,
                        m.npl_estimate(),
                        nfm_core::model::PLANE_CAP
                    );
                }
            }
            Err(e) => {
                eprintln!("{e}");
                failures += 1;
            }
        }
    }

    if failures == 0 {
        println!(
            "// {} models parsed clean (planes = <p> polygons; original npl also caches 9 planes per wheel)",
            zipo.entries.len()
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("// {failures} model(s) failed to parse");
        ExitCode::FAILURE
    }
}
