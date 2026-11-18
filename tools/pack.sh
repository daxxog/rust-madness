#!/bin/bash
# Pack every play-time resource into ONE derived zip at the workspace root:
# the recovered game assets plus the shared glyph atlas the text renderer
# reads. `crates/nfm-core/build.rs` embeds that file into the binary
# (`NFM_PACK_PATH`), and `pack.rs` serves it from memory — a release build
# then reads no resource file from the filesystem.
#
# The pack is DERIVED: it is gitignored and never committed. Re-run this
# after changing anything under assets/ or tests/fixtures/xtfont-atlas/;
# build.rs watches the archive, so the next cargo build re-embeds it.
#
# Entry names are repo-root-relative, e.g.
#   assets/graphics/models.zipo
#   tests/fixtures/xtfont-atlas/atlas.txt
set -euo pipefail
cd "$(dirname "$0")/.."

rm -f resources.pack
# -X: no extra per-entry attributes (a cleaner, reproducible archive)
# -D: no directory entries — only real files, each named from this root
zip -q -r -X -D resources.pack assets tests/fixtures/xtfont-atlas

entries=$(unzip -Z1 resources.pack | wc -l | tr -d ' ')
bytes=$(wc -c <resources.pack | tr -d ' ')
echo "resources.pack: $entries entries, $bytes bytes"

# build.rs embeds the archive through include_bytes!(env!("NFM_PACK_PATH")).
# Once a pack exists its own mtime drives re-embeds; for the FIRST pack of a
# tree that was built without one, bump build.rs's mtime so cargo reruns the
# script (a missing rerun-if-changed path would recompile on every build).
touch crates/nfm-core/build.rs
