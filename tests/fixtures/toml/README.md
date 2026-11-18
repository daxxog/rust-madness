# The fixture TOML contract

The files in this directory are the **single source of truth** for the static
values the behavior tests consume: magic integers, hardcoded tables, md5 pins,
determinism vectors, seeds, walk scripts and scenario scripts. BOTH test
suites read them at runtime:

- **Java**: `tests/java/Toml.java` — a deliberately minimal TOML-subset
  reader (see below). No dependencies; anything outside the subset fails
  loudly with file, line and key path.
- **Rust (Phase B)**: the real `toml` crate, as a dev/test-support
  dependency only — `nfm-core` itself stays zero-new-dependency.

The TOML values are **normative**; the B-docs (notes/JAVA-BEHAVIOR/B*.md)
stay the narrative record (sources, derivations, corrections) and may restate
values for readability, but on any apparent conflict the TOML wins. The
migration trail: values were transcribed from the test classes verbatim, and
every gate run proves transcription fidelity (pin counts byte-equal, every
md5 identical).

## The subset

Exactly what `Toml.java` parses — fixture files MUST stay inside it:

```
[table]  [table.sub]  [[array.of.tables]]  (nesting: an intermediate
          component naming an [[array]] binds into its most recent row,
          the standard TOML rule - see the [[scenario]]/[[scenario.phase]]
          example in scenarios.toml)
key = 123 | -4 | 1.5 | 1.5e-3 | "text" | true | false
key = [1, 2, 3] | ["a", "b"]          # ONE-LINE arrays only
# comments to end of line; blank lines ignored
```

Banned (the Java reader rejects them): dotted keys, inline tables, multi-line
arrays/strings, dates/times, hex/octal/binary, underscores and leading `+` in
numbers, NaN/Infinity, UTF-8 BOM. Bare keys are `[A-Za-z0-9_-]+`; all ids are
snake_case and **stable forever** — they are the shared checklist the Rust
mirror pins against, renaming one is a breaking change to both suites.

Numbers: integers parse as 64-bit and are range-checked into `int` at the
lookup the game needs (the original code is int math); floats are IEEE
decimal literals. Strings: double-quoted, `\\ \" \n \t \r` escapes only.

## Files and ownership

| file | owns | consumers |
|---|---|---|
| `audio.toml` | the 11 stage tracks: RadicalMod args, decode md5+length, mix md5+calls+samples, structural rows | TAudioGoldens (Java), the Rust AudioBus tests |
| `pixel.toml` | the pixel manifest: atlas geometry + format + sha256s, font-probe md5s, the `[frames]` reference block and the `[frame]` canvas geometry | PixelSuite/PixelRace (Java), the Rust raster tests |
| `determinism.toml` | seeds, repeating random vectors, preset-walk recipes, recorded walks | TAudioGoldens-adjacent logic classes, PixelSuite, the Rust mirror |
| `constants.toml` | load-bearing game constants: clamps, thresholds, colors, band indices, ladder values | the logic T-classes, the Rust mirror |
| `scenarios.toml` | drive-scenario scripts: control-input phases + tick expectations | TMadnessDrive (Java), the Rust drive tests |

**The pixel two-file contract**: the 83 frame md5 rows stay in
`tests/pixel/expected.md5` — that file is the `--bless` artifact and the
pixel gate's row source, untouched by this migration. `pixel.toml` does NOT
duplicate the rows; its `[frames]` block references the file (`file`,
`row_count`) so both suites resolve the manifest the same way. Frame DATA
lives in expected.md5; everything else pixel lives here, including the
`[frame]` w/h (550x400) every pinned frame is rendered at.

## Consumption rules

1. Every consumer asserts its shapes at load (`requireKeys` / `tableCount`),
   and those assertions are pins — a malformed fixture fails loudly at the
   first pin, never silently. Adding a row means updating the consumer's
   shape pin in the same change.
2. Values are immutable once the suite re-freezes (T3 of the migration):
   any later change is a batch with a gate run and a doc trail, same as the
   original pin discipline.
3. Transcription must be verbatim from the test class being migrated; the
   gate's count/md5 equality is the proof. A value that disagrees with a
   B-doc is a CORRECTION: fix both, cite the source, and record it in the
   B-doc (the standing report rule).
4. Source citations live in TOML comments (`# Madness.java:1420`), never as
   data keys.
5. No new files in this directory without a README row above.
