#!/usr/bin/env bash
# Compiles each decompiled reference tree with the container's javac.
# This is a syntax/round-trip sanity check only: failure to compile usually
# means the decompiler emitted broken code for the ancient bytecode, which is
# exactly the kind of thing we need to know about. Errors land in
# reference/logs/<tool>.javac.log.
set -u

OUT=/work/reference
LOGS="$OUT/logs"

check_tree() {
  local name="$1" dir="$OUT/$1"
  local list n b errors
  list="$(mktemp)"
  find "$dir" -name '*.java' | sort > "$list"
  n=$(wc -l < "$list" | tr -d ' ')
  if [ "$n" -eq 0 ]; then
    echo "  $name: no .java sources found, skipping"
    rm -f "$list"
    return
  fi
  b="/tmp/check-$name"
  rm -rf "$b"
  mkdir -p "$b"
  # javac-stubs carries era-specific API stubs missing from modern JDKs
  # (JDK 1.x internal sun.audio, Java-plugin netscape.javascript.JSObject).
  # --limit-modules keeps jdk.jsobject unobservable so our JSObject stub
  # does not collide with the abstract JDK-internal one.
  if javac --limit-modules java.base,java.desktop -nowarn -proc:none -d "$b" \
       $(find /opt/tools/javac-stubs -name '*.java' | tr '\n' ' ') @"$list" \
       > "$LOGS/$name.javac.log" 2>&1; then
    echo "  $name: OK ($n files)"
  else
    errors=$(grep -c 'error:' "$LOGS/$name.javac.log" || true)
    echo "  $name: FAILED ($errors errors -> reference/logs/$name.javac.log)"
  fi
  rm -f "$list"
}

for t in cfr vineflower procyon; do
  check_tree "$t"
done
