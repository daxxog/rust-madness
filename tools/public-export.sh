#!/bin/bash
# tools/public-export.sh — materialize the PUBLIC GitHub repo as a filtered
# export of this private source tree.
#
# THE PUBLIC REPO IS DERIVED. Never edit it by hand after materialization:
# every change flows from this source tree through this script. Re-run to
# converge; DEST/.git is never touched.
#
# The exported subset is exactly:
#   crates/ tests/ assets/ tools/ mad.jar Cargo.toml Cargo.lock public/
# with public/* overlaid onto the DEST root (public/README.md -> README.md,
# public/.gitignore -> .gitignore, public/.github/ -> .github/, ...).
# reference/ and notes/ never leave this repository, with ONE exception:
# reference/external/catb0t/OBJ/mycars/Simple Car.rad is a citable third-party
# test fixture (not decompiled output) that the Java and Rust gates parse.
#
# Usage: tools/public-export.sh [DEST]
#   DEST defaults to ../madness-public relative to this repo's root. A relative
#   DEST argument is also resolved against that root, so the script works from
#   any cwd.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
DEST_INPUT="${1:-../madness-public}"
case "$DEST_INPUT" in
  /*) DEST="$DEST_INPUT" ;;
  *)  DEST="$ROOT/$DEST_INPUT" ;;
esac

die() { echo "public-export: ERROR: $*" >&2; exit 1; }

# The sole reference/ exception: catb0t's dev-game model, used as a byte-parse
# fixture by tests/java/TContO.java and nfm-core's mirror::mirror_suite.
FIXTURE_REL='reference/external/catb0t/OBJ/mycars/Simple Car.rad'

# --- preflight: fail before touching DEST ----------------------------------
[[ "$DEST" != "$ROOT" ]] || die "DEST must not be the source repository root"
command -v rsync >/dev/null 2>&1 || die "rsync is required"

[[ -f "$ROOT/mad.jar" ]] || die "mad.jar is missing from the source tree"
[[ -f "$ROOT/Cargo.toml" && -f "$ROOT/Cargo.lock" ]] || die "Cargo.toml / Cargo.lock missing from the source tree"
[[ -d "$ROOT/public" ]] || die "public/ overlay directory is missing"

# The license is a pending user decision; a repo cannot ship without one.
[[ -f "$ROOT/public/LICENSE" ]] || die \
  "public/LICENSE is absent: the license for the public repository is a PENDING USER DECISION. Choose and add public/LICENSE before exporting; this script will not ship a repository without one."

for crate in nfm-core nfm-dump nfm-render; do
  [[ -f "$ROOT/crates/$crate/Cargo.toml" ]] || die "crates/$crate/ is missing or incomplete"
done

mkdir -p "$DEST"
DEST="$(cd "$DEST" && pwd -P)"

# --- sync: per-directory --delete makes re-runs converge --------------------
for dir in crates tests assets tools public; do
  [[ -d "$ROOT/$dir" ]] || die "source directory $dir/ is missing"
  mkdir -p "$DEST/$dir"
  rsync -a --delete "$ROOT/$dir/" "$DEST/$dir/" || die "rsync failed for $dir/"
done
for file in mad.jar Cargo.toml Cargo.lock; do
  cp -p "$ROOT/$file" "$DEST/$file" || die "failed to copy $file"
done

# overlay public/* onto the DEST root (dotfiles and dot-directories too:
# .gitignore, .github/ - the latter is a tree, so sync it instead of cp)
for src in "$ROOT"/public/.[!.]* "$ROOT"/public/*; do
  [[ -e "$src" ]] || continue
  base="$(basename "$src")"
  [[ "$base" == "." || "$base" == ".." ]] && continue
  if [[ -d "$src" ]]; then
    mkdir -p "$DEST/$base"
    rsync -a --delete "$src/" "$DEST/$base/" || die "overlay failed for $base"
  else
    cp -p "$src" "$DEST/$base" || die "overlay failed for $base"
  fi
done

# --- explicit cleanup of hard exclusions (stale DEST junk removed) ----------
# DEST/.git is never matched or touched.
EXCLUDED=(reference notes target render nfm-cookies.txt resources.pack .cross-deck.toml .DS_Store 'nfm.log*' 'nfm-probe.*')
for pat in "${EXCLUDED[@]}"; do
  find "$DEST" -name .git -prune -o -name "$pat" -prune -exec rm -rf {} + 2>/dev/null || true
done

# --- carve-out: the one third-party fixture the gates need -------------------
# reference/ is otherwise a hard exclusion and CI re-derives the decompile;
# this single citable catb0t model is a TEST FIXTURE, not decompiled output.
[[ -f "$ROOT/$FIXTURE_REL" ]] || die "fixture missing from source: $FIXTURE_REL"
mkdir -p "$(dirname "$DEST/$FIXTURE_REL")"
cp -p "$ROOT/$FIXTURE_REL" "$DEST/$FIXTURE_REL" || die "failed to copy fixture"

# --- verification -----------------------------------------------------------
echo "public-export: verifying $DEST"

for pat in "${EXCLUDED[@]}"; do
  hit="$(find "$DEST" -name .git -prune -o -name "$pat" -print -quit 2>/dev/null)"
  if [[ "$pat" == "reference" && "$hit" == "$DEST/reference" ]]; then
    # Only the carve-out and its ancestor directories may live here.
    extra=''
    while IFS= read -r p; do
      rel="${p#$DEST/}"
      case "$FIXTURE_REL" in
        "$rel"|"$rel"/*) ;; # the fixture itself, or one of its parent dirs
        *) extra="$extra $rel" ;;
      esac
    done < <(find "$DEST/reference" -print)
    [[ -z "$extra" ]] || die "extraneous path(s) in reference/ carve-out:$extra"
  else
    [[ -z "$hit" ]] || die "hard exclusion violated: '$pat' found at $hit"
  fi
done
echo "  exclusions: OK (reference/ except the cited fixture, notes/, target/, render/, nfm-cookies.txt, resources.pack, nfm.log*, nfm-probe.*, .cross-deck.toml, .DS_Store)"

cmp -s "$ROOT/mad.jar" "$DEST/mad.jar" || die "mad.jar did not copy byte-identically"
echo "  mad.jar: OK (byte-identical, sha256 $(shasum -a 256 "$DEST/mad.jar" | awk '{print $1}'))"

cmp -s "$ROOT/$FIXTURE_REL" "$DEST/$FIXTURE_REL" || die "fixture did not copy byte-identically"
echo "  fixture: OK (byte-identical, sha256 $(shasum -a 256 "$DEST/$FIXTURE_REL" | awk '{print $1}'))"

crate_files=0
for crate in nfm-core nfm-dump nfm-render; do
  [[ -d "$DEST/crates/$crate" ]] || die "crates/$crate/ did not copy"
  n="$(find "$DEST/crates/$crate" -type f | wc -l | tr -d ' ')"
  [[ "$n" -gt 0 ]] || die "crates/$crate/ copied empty"
  crate_files=$((crate_files + n))
done
echo "  crates: OK ($crate_files files across nfm-core, nfm-dump, nfm-render)"

for file in README.md NOTICES.md PROVENANCE.md .gitignore LICENSE public/README.md; do
  [[ -f "$DEST/$file" ]] || die "overlay file $file is missing from the export"
done
echo "  overlay: OK (README.md, NOTICES.md, PROVENANCE.md, .gitignore, LICENSE)"

# Path scan only: tools/java-tests.sh legitimately mentions reference/vineflower
# in its CONTENTS; no exported FILE PATH may contain either string, except the
# single cited fixture and the ancestor dirs needed to hold it.
bad_paths=''
while IFS= read -r p; do
  rel="${p#./}"
  case "$FIXTURE_REL" in
    "$rel"|"$rel"/*) continue ;; # the explicit third-party test-fixture exception
  esac
  bad_paths="$bad_paths $rel"
done < <(cd "$DEST" && find . -name .git -prune -o -print | grep -E 'vineflower|reference/' || true)
[[ -z "$bad_paths" ]] || die "forbidden path(s) in export:$bad_paths"
echo "  forbidden-path scan: OK (no 'vineflower' or 'reference/' except the cited Simple Car.rad fixture)"

# --- summary ----------------------------------------------------------------
n_files="$(find "$DEST" -name .git -prune -o -type f -print | wc -l | tr -d ' ')"
n_bytes="$(find "$DEST" -name .git -prune -o -type f -exec stat -f%z {} + | awk '{s += $1} END {printf "%.0f", s + 0}')"
echo "public-export: OK"
echo "  destination: $DEST"
echo "  files: $n_files"
echo "  bytes: $n_bytes"
echo "  top-level layout:"
for entry in "$DEST"/* "$DEST"/.[!.]*; do
  [[ -e "$entry" ]] || continue
  printf '    %s\n' "$(basename "$entry")"
done | sort -u
