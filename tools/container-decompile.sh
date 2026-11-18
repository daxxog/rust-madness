#!/usr/bin/env bash
# Runs inside the container (see tools/decompile.sh): decompiles /work/mad.jar
# with every tool into /work/reference/<tool>/, plus javap disassembly and a
# javac round-trip syntax check of each decompiled tree.
set -u

SRC=/work/mad.jar
OUT=/work/reference
LOGS="$OUT/logs"

mkdir -p "$OUT" "$LOGS"

echo "==> environment"
java -version > "$LOGS/java-version.log" 2>&1
echo "    $(head -1 "$LOGS/java-version.log")"
echo "    CFR=$CFR_VERSION  Vineflower=$VINEFLOWER_VERSION  Procyon=$PROCYON_VERSION"
jar tf "$SRC" > "$LOGS/jar-listing.txt"
echo "    jar entries: $(wc -l < "$LOGS/jar-listing.txt" | tr -d ' ')"

run_tool() {
  local name="$1"
  shift
  echo "==> decompile: $name"
  if "$@" > "$LOGS/$name.log" 2>&1; then
    echo "    ok  ($(find "$OUT/$name" -name '*.java' 2>/dev/null | wc -l | tr -d ' ') .java files)"
  else
    echo "    FAILED  (see reference/logs/$name.log)"
  fi
}

# Clean previous outputs so the run is always reproducible/regenerable.
rm -rf "$OUT/cfr" "$OUT/vineflower" "$OUT/procyon" "$OUT/javap" "$OUT/javap-v" "$OUT/assets"
for d in cfr vineflower procyon javap javap-v assets; do mkdir -p "$OUT/$d"; done

run_tool cfr \
  java -jar "$CFR_JAR" --silent true --outputdir "$OUT/cfr" "$SRC"

run_tool vineflower \
  java -jar "$VINEFLOWER_JAR" --silent "$SRC" "$OUT/vineflower"

# Procyon: Maven Central's compilertools jar has no CLI, so PxDriver (baked
# into the image by the Dockerfile) drives the library API: one file per class.
# compilertools needs procyon-core on the runtime classpath.
run_tool procyon \
  java -cp "$PROCYON_JAR:$PROCYON_CORE_JAR:/opt/tools" PxDriver "$SRC" "$OUT/procyon" "$OUT/logs/procyon-failures.log"

echo "==> disassembly (javap)"
n=0
for c in $(jar tf "$SRC" | sed -n 's/\.class$//p' | sort); do
  javap -p -c -s -constants -classpath "$SRC" "$c" > "$OUT/javap/$c.javap" 2>&1 || echo "    javap failed: $c"
  javap -p -v -classpath "$SRC" "$c"  > "$OUT/javap-v/$c.javap-v" 2>&1 || echo "    javap -v failed: $c"
  n=$((n + 1))
done
echo "    wrote $n class disassemblies"

echo "==> extract bundled assets"
(cd "$OUT/assets" && jar xf "$SRC" $(jar tf "$SRC" | sed -n 's/\.gif$/&/p' | tr '\n' ' ')) \
  && echo "    $(ls "$OUT/assets" | wc -l | tr -d ' ') assets"

echo "==> round-trip javac check of decompiled trees"
/usr/local/bin/container-check-compile.sh

echo "==> done (decompiled sources + disassembly in reference/)"
