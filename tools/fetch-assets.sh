#!/usr/bin/env bash
# Recover the game's server-side asset bundle (the parts mad.jar fetches at
# runtime) from the Wayback Machine:
#
#   assets/graphics/{models,images1..4}.zipo   models (.rad) + UI gifs (zips)
#   assets/music/{stage1..11,cars,stages}.zipo ProTracker .mod modules (zips)
#   assets/stages/{1..11}.txt                  stage definition text
#   assets/sounds/newsun/*.au                  Sun .au sound effects (clips)
#
# For each resource we ask the CDX API for its captures, take the earliest
# 200-OK one (closest to the 2005/2007 applet era), and download the ORIGINAL
# bytes via the /id_/ memento (no wayback banner injection). Every file is
# verified (zip magic / non-empty) and its sha1 is re-encoded the same way
# Wayback's CDX digests are (base32) and compared against the archive's own
# digest for that capture — byte-identity proof. The result is recorded in
# assets/PROVENANCE.md (regenerated fully on every run).
#
# The Internet Archive intermittently rate-limits or goes briefly offline,
# so requests retry with backoff and files already present are skipped
# (rerunning is cheap; delete a file to force a refetch).
#
#   tools/fetch-assets.sh
set -uo pipefail
cd "$(dirname "$0")/.."

BASE="http://www.radicalplay.com/madness"
CDX="https://web.archive.org/cdx/search/cdx"

RESOURCES=(
  graphics/models.zipo
  graphics/images1.zipo
  graphics/images2.zipo
  graphics/images3.zipo
  graphics/images4.zipo
  music/cars.zipo
  music/stages.zipo
  music/stage1.zipo
  music/stage2.zipo
  music/stage3.zipo
  music/stage4.zipo
  music/stage5.zipo
  music/stage6.zipo
  music/stage7.zipo
  music/stage8.zipo
  music/stage9.zipo
  music/stage10.zipo
  music/stage11.zipo
  stages/1.txt
  stages/2.txt
  stages/3.txt
  stages/4.txt
  stages/5.txt
  stages/6.txt
  stages/7.txt
  stages/8.txt
  stages/9.txt
  stages/10.txt
  stages/11.txt
  sounds/newsun/a0.au
  sounds/newsun/a1.au
  sounds/newsun/a2.au
  sounds/newsun/a3.au
  sounds/newsun/a4.au
  sounds/newsun/b0.au
  sounds/newsun/b1.au
  sounds/newsun/b2.au
  sounds/newsun/b3.au
  sounds/newsun/b4.au
  sounds/newsun/air0.au
  sounds/newsun/air1.au
  sounds/newsun/air2.au
  sounds/newsun/air3.au
  sounds/newsun/air4.au
  sounds/newsun/air5.au
  sounds/newsun/crash1.au
  sounds/newsun/crash2.au
  sounds/newsun/crash3.au
  sounds/newsun/lowcrash1.au
  sounds/newsun/lowcrash2.au
  sounds/newsun/lowcrash3.au
  sounds/newsun/tires.au
  sounds/newsun/checkpoint.au
  sounds/newsun/carfixed.au
  sounds/newsun/powerup.au
  sounds/newsun/three.au
  sounds/newsun/two.au
  sounds/newsun/one.au
  sounds/newsun/go.au
  sounds/newsun/skid1.au
  sounds/newsun/skid2.au
  sounds/newsun/dustskid1.au
  sounds/newsun/dustskid2.au
  sounds/newsun/wasted.au
  sounds/newsun/firewasted.au
)

# fetch with retries/backoff; success clears $? silently, failure print/return 1
http_get() {
  local url="$1" tries=6 wait=10 rc
  while :; do
    rc=0
    curl -sL --fail --retry 0 -m 120 "$url" || rc=$?
    if [ "$rc" -eq 0 ]; then
      return 0
    fi
    echo "    get failed (curl $rc), retrying in ${wait}s..." >&2
    sleep "$wait"
    wait=$((wait * 2))
    tries=$((tries - 1))
    if [ "$tries" -le 0 ]; then
      echo "ERROR: giving up on $url" >&2
      return 1
    fi
  done
}

# earliest 200 capture for path -> writes TS and DIGEST globals
cdx_earliest() {
  local path="$1" tries=5 wait=10 row
  while :; do
    row=$(curl -sL -m 60 "$CDX?url=radicalplay.com/madness/$path&fl=timestamp,statuscode,digest&limit=1" 2>/dev/null \
      | grep -E '^[0-9]{14} 200 [A-Z2-7]{32}' | head -1)
    if [ -n "$row" ]; then
      TS=$(echo "$row" | cut -d' ' -f1)
      DIGEST=$(echo "$row" | cut -d' ' -f3)
      return 0
    fi
    echo "    cdx returned nothing usable for $path (IA hiccup?), retrying in ${wait}s..." >&2
    sleep "$wait"
    wait=$((wait * 2))
    tries=$((tries - 1))
    if [ "$tries" -le 0 ]; then
      echo "ERROR: no captures found for $path" >&2
      return 1
    fi
  done
}

verify_file() {
  local file="$1"
  if [[ "$file" == *.zipo ]]; then
    # zip magic PK\x03\x04, compared as hex so no binary-hostile tooling
    [ "$(xxd -l 4 -p "$file" 2>/dev/null)" = "504b0304" ]
  elif [[ "$file" == *.au ]]; then
    # Sun .au magic ".snd", compared as hex the same way
    [ "$(xxd -l 4 -p "$file" 2>/dev/null)" = "2e736e64" ]
  else
    [ -s "$file" ]
  fi
}

user_sha1_b32() { # file -> content sha1, base32 (same encoding as CDX digests)
  python3 - "$1" <<'EOF'
import base64, hashlib, sys
h = hashlib.sha1(open(sys.argv[1], "rb").read()).digest()
print(base64.b32encode(h).decode())
EOF
}

mkdir -p assets/graphics assets/music assets/stages assets/sounds/newsun
ROWS="$(mktemp)"

OK=0; FAIL=0
for rel in "${RESOURCES[@]}"; do
  out="assets/$rel"
  echo "== $rel"
  if ! cdx_earliest "$rel"; then
    FAIL=$((FAIL + 1))
    continue
  fi
  if [ -s "$out" ] && verify_file "$out"; then
    echo "    already present (capture $TS)"
  else
    echo "    fetching capture $TS"
    if http_get "https://web.archive.org/web/${TS}id_/${BASE}/$rel" > "$out" && verify_file "$out"; then
      OK=$((OK + 1))
    else
      echo "    fetch/verify FAILED, removing $out" >&2
      rm -f "$out"
      FAIL=$((FAIL + 1))
      continue
    fi
  fi
  local_digest=$(user_sha1_b32 "$out")
  size=$(wc -c < "$out" | tr -d ' ')
  sha256=$(shasum -a 256 "$out" | cut -d' ' -f1)
  match="MISMATCH"; [ "$local_digest" = "$DIGEST" ] && match="match"
  printf '| `%s` | %s | %s | `%s` | %s | %s |\n' "$rel" "$TS" "$size" "$local_digest" "$sha256" "$match" >> "$ROWS"
  sleep 2
done

# --- the loose codebase gifs, recovered from mad.jar -----------------------
# hello.gif (GIF87a), sign.gif + loadbar.gif (GIF89a) ship inside mad.jar and
# load, at runtime, from the codebase root (`getImage(getCodeBase(), name)`).
JAR_ROWS="$ROWS.2"
LOOSE_OK=0; LOOSE_FAIL=0
if [ -s mad.jar ]; then
  for name in hello.gif sign.gif loadbar.gif; do
    out="assets/graphics/$name"
    tmp=$(mktemp)
    echo "== $name (mad.jar member)"
    unzip -p mad.jar "$name" > "$tmp" || { echo "ERROR: $name missing from mad.jar" >&2; LOOSE_FAIL=$((LOOSE_FAIL + 1)); continue; }
    magic=$(xxd -l 3 -p "$tmp")
    case "$magic" in 474946) : ;; # "GIF"
      *) echo "ERROR: bad GIF magic in $name" >&2; LOOSE_FAIL=$((LOOSE_FAIL + 1)); rm -f "$tmp"; continue ;;
    esac
    if ! cmp -s "$tmp" "$out" || [ ! -s "$out" ]; then
      cp "$tmp" "$out"
    fi
    LOOSE_OK=$((LOOSE_OK + 1))
    local_digest=$(user_sha1_b32 "$out")
    size=$(wc -c < "$out" | tr -d ' ')
    sha256=$(shasum -a 256 "$out" | cut -d' ' -f1)
    printf '| `graphics/%s` | mad.jar | %s | `%s` | %s |\n' "$name" "$size" "$local_digest" "$sha256" >> "$JAR_ROWS"
    rm -f "$tmp"
  done
else
  echo "ERROR: mad.jar not found; skipping loose gif recovery" >&2
fi

{
  echo "# assets/PROVENANCE.md"
  echo
  echo "Regenerated by \`tools/fetch-assets.sh\` on $(date -u +'%Y-%m-%d %H:%M UTC')."
  echo
  echo "For each resource below: the **earliest known 200-OK capture** in the"
  echo "Wayback Machine, downloaded via the \`/id_\` memento so these are the"
  echo "original bytes (no wayback decoration). The sha1 column is the local"
  echo "content SHA-1 encoded in the same base32 alphabet Wayback's CDX digests"
  echo "use; \`match\` proves byte-identity with the archived capture."
  echo
  echo "| resource | capture | bytes | sha1 (base32) | sha256 | digest check |"
  echo "|---|---|---|---|---|---|"
  cat "$ROWS"
  echo
  # The three loose loading-screen gifs load codebase-relative at runtime and
  # are additionally bundled in mad.jar; no codebase captures exist, so they
  # recover from the jar's own copies.
  echo "The three loose gifs below load from the applet codebase at runtime"
  echo "(\`getImage(getCodeBase(), ...\`, xtGraphics:2937-2951). No Wayback"
  echo "codebase captures exist, so they are recovered from the copies bundled"
  echo "in \`mad.jar\` (verified GIF87a/GIF89a magic)."
  echo
  echo "| resource | source | bytes | sha1 (base32) | sha256 |"
  echo "|---|---|---|---|---|"
  cat "$JAR_ROWS"
  echo
} > assets/PROVENANCE.md
rm -f "$ROWS"

echo
MISMATCHES=$(grep -c '| MISMATCH |' assets/PROVENANCE.md || true)
echo "recovered $OK resources ($FAIL failed, $MISMATCHES digest mismatches); loose gifs $LOOSE_OK ok ($LOOSE_FAIL failed)"
[ "$FAIL" -eq 0 ] && [ "$MISMATCHES" -eq 0 ] && [ "$LOOSE_FAIL" -eq 0 ] || exit 1
