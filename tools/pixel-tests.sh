#!/bin/bash
# The pixel-test gate: builds the Temurin 8 + Xvfb pixel image, compiles the
# DECOMPILED REFERENCE SOURCES UNMODIFIED together with the java and pixel
# tests, and runs PixelSuite - all inside podman, so the raster is the
# container's exact raster and nothing depends on the mac's fonts, JDK or
# display. The repo is mounted READ-ONLY at /w (zero-modification enforced
# structurally); the shipped text font rides that mount from tools/fonts/.
#
# Contract: era-exactness is APPROXIMATE (the 2005 JRE platform fonts are
# gone; the shipped Liberation Sans Bold stands in for logical SansSerif),
# REPRODUCIBILITY is EXACT (one image, one font file, one fontconfig
# mapping, md5-pinned frames). See notes/JAVA-BEHAVIOR/B12_pixel.md.
#
# Usage:
#   tools/pixel-tests.sh           # verify against tests/pixel/expected.md5
#   tools/pixel-tests.sh --bless   # rewrite expected.md5 (intentional change)
#   tools/pixel-tests.sh --gen     # regenerate tests/fixtures/xtfont-atlas/
#
# Only --gen and --bless write to the repo, and only into their own artifact
# directory (a second, narrow rw mount; the source tree stays ro).
set -euo pipefail
cd "$(dirname "$0")/.."

MODE="${1:-}"
case "$MODE" in
  ""|--verify|--bless|--gen) ;;
  *) echo "usage: tools/pixel-tests.sh [--verify|--bless|--gen]" >&2; exit 2 ;;
esac

JDK8="${JAVA8_HOME:-/Library/Java/JavaVirtualMachines/jdk1.8.0_40.jdk/Contents/Home}"
# the real netscape.javascript classes (GameSparker's import); on
# OpenJDK/Temurin this jar must ride in from an Oracle 8 install. Staged in
# target/java-tools (NOT target/java-tests: the coverage CLI walks directories
# recursively and would analyze the plugin's classes).
PLUGIN="$JDK8/jre/lib/plugin.jar"
# see tools/java-tests.sh: plugin.jar's JSObject needs deploy.jar or the
# class throws an uncatchable NoClassDefFoundError on Linux
DEPLOY="$JDK8/jre/lib/deploy.jar"
mkdir -p target/java-tools
cp "$PLUGIN" target/java-tools/plugin.jar
cp "$DEPLOY" target/java-tools/deploy.jar

podman build -q -f tools/pixel-tests.Dockerfile -t nfm-java8-pixel >/dev/null

RUN=(podman run --rm -v "$PWD:/w:ro" -v "$PWD/target:/w/target:rw")
case "$MODE" in
  --gen)
    mkdir -p tests/fixtures/xtfont-atlas
    RUN+=(-v "$PWD/tests/fixtures/xtfont-atlas:/out")
    ;;
  --bless)
    mkdir -p tests/pixel
    RUN+=(-v "$PWD/tests/pixel:/out")
    ;;
esac

exec "${RUN[@]}" -e PIXEL_MODE="$MODE" nfm-java8-pixel bash -ec '
  Xvfb :99 -screen 0 800x600x24 >/dev/null 2>&1 &
  sleep 1
  mkdir -p /b
  javac -encoding UTF-8 -cp /w/target/java-tools/plugin.jar:/w/target/java-tools/deploy.jar -d /b \
    /w/reference/vineflower/*.java /w/tests/java/*.java /w/tests/pixel/*.java
  cd /w
  FONT=-Dsun.awt.fontconfig=/w/tools/fonts/fontconfig.properties
  case "$PIXEL_MODE" in
    --gen)
      DISPLAY=:99 java -Djava.awt.headless=false $FONT \
        -Datlas.dir=/out -cp "/b:/w/target/java-tools/plugin.jar:/w/target/java-tools/deploy.jar" PixelSuite --gen
      ;;
    --bless)
      DISPLAY=:99 java -Djava.awt.headless=false $FONT \
        -Dexpected.out=/out/expected.md5 -cp "/b:/w/target/java-tools/plugin.jar:/w/target/java-tools/deploy.jar" PixelSuite --bless
      ;;
    *)
      DISPLAY=:99 java -Djava.awt.headless=false $FONT \
        -cp "/b:/w/target/java-tools/plugin.jar:/w/target/java-tools/deploy.jar" PixelSuite
      ;;
  esac
'
