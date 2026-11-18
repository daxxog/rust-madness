//! The Phase B mirror suite (notes/PHASE-B.md): the Java behavior
//! suite's pins replayed 1:1 against this port, consuming the same
//! `tests/fixtures/toml/*.toml` fixtures and the same ground truths.
//!
//! Layout: one module per Java T-class with a `run()` each, executed in
//! `Suite.java` order by [`mirror_suite`] — the single report line diffs
//! mechanically against the Java gate's. A module lands as its batch
//! completes; the class list below is the live tracker.

pub(crate) mod fixture;
pub(crate) mod harness;
mod pixel;
mod sha256;
mod t_audio_goldens;
mod t_conto;
mod t_contodraw;
mod t_control;
mod t_gamesparker_logic;
mod t_java_random;
mod t_madness;
mod t_madness_drive;
mod t_madness_world;
mod t_medium;
mod t_mod;
mod t_mod_edges;
mod t_modslayer;
mod t_plane;
mod t_radicalmod;
mod t_record;
mod t_small;
mod t_ulaw_utils;
mod t_wheels;
mod t_xt_graphics_logic;
mod util;

#[test]
fn mirror_suite() {
    // Suite.java order. ALL 20 classes are landed (batch 11 closed the
    // campaign): TMod, TUlawUtils, TMedium, TModSlayer, TMadness,
    // TMadnessWorld, TSmall, TJavaRandom, TRadicalMod, TWheels, TRecord,
    // TContO, TContODraw, TPlane, TControl, TGameSparkerLogic,
    // TxtGraphicsLogic, TAudioGoldens, TMadnessDrive, TModEdges.
    t_mod::run();
    t_ulaw_utils::run();
    t_medium::run();
    t_modslayer::run();
    t_madness::run();
    t_madness_world::run();
    t_small::run();
    t_java_random::run();
    t_radicalmod::run();
    t_wheels::run();
    t_record::run();
    t_conto::run();
    t_contodraw::run();
    t_plane::run();
    t_control::run();
    t_gamesparker_logic::run();
    t_xt_graphics_logic::run();
    t_audio_goldens::run();
    t_madness_drive::run();
    t_mod_edges::run();
    fixture::smoke();

    let rc = harness::report();
    assert_eq!(rc, 0, "mirror pins failed - see the FAIL lines above");
}

/// The SECOND Java suite: `tests/pixel/PixelSuite.java` (B12). Batch C1 landed
/// its atlas + font-probe sections and C3a the screens section (40 of the 42
/// `screen_*` rows; the clicknow pair is the report's 12pt fixture blocker),
/// with the race rows following in C3b. Its own report line diffs against the
/// Java pixel gate's, so the two suites stay separately measurable.
///
/// `PixelSuite.main` installs the seeded `Math.random()` slot as its first
/// game-relevant statement (`:118-126`); the runner replays that order: read
/// the seed with the TOML loader (harness code, consumes no randomness), then
/// scope the whole run in the injection.
#[test]
fn mirror_pixel_suite() {
    let seed = fixture::Tml::load("determinism.toml")
        .table("seed")
        .longv("pixel_main");
    let _guard = crate::random::install(seed);
    pixel::run();
    let rc = harness::report();
    assert_eq!(rc, 0, "pixel pins failed - see the FAIL lines above");
}
