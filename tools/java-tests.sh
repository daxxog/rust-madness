#!/bin/bash
# Compiles the DECOMPILED REFERENCE SOURCES UNMODIFIED together with the
# java behavior tests, then runs the suite. The original lines are taken
# from reference/vineflower/ exactly as decompiled - the tests live in
# tests/java/ and the reference tree is never written to.
#
# Usage:
#   tools/java-tests.sh             # local JDK 8 (Oracle 8u40)
#   tools/java-tests.sh --container # the identical suite in podman
#                                   # Temurin 8 - passing on both
#                                   # builds is itself a fidelity check
set -euo pipefail
cd "$(dirname "$0")/.."

JDK8="${JAVA8_HOME:-/Library/Java/JavaVirtualMachines/jdk1.8.0_40.jdk/Contents/Home}"
# the real netscape.javascript classes (GameSparker's import); on
# OpenJDK/Temurin this jar must ride in from an Oracle 8 install
PLUGIN="$JDK8/jre/lib/plugin.jar"
# plugin.jar's JSObject.getWindow links com.sun.deploy.appcontext.AppContext
# (deploy.jar) - without it the class throws NoClassDefFoundError, a
# LinkageError the source's catch (Exception) sites cannot catch, which
# killed every run()-based test in the container. Locally the mac Oracle
# JVM resolves that path without help, so the local classpath stays
# plugin.jar-only (changing it would churn green pins).
DEPLOY="$JDK8/jre/lib/deploy.jar"

SRC_REF="reference/vineflower"
SRC_TEST="tests/java"

if [[ "${1:-}" == "--container" ]]; then
  # The repo is mounted READ-ONLY: zero-modification enforced
  # structurally. The image carries Xvfb + fonts (the original code
  # demands headful AWT). plugin.jar rides in from the local Oracle
  # 8u40 install - staged under target/java-tools (NOT
  # target/java-tests: the coverage CLI walks --classfiles recursively
  # and would otherwise analyze the plugin's 686 classes into the
  # report). podman's machine only shares /Users-family paths with the
  # mac, not /Library.
  mkdir -p target/java-tools
  cp "$PLUGIN" target/java-tools/plugin.jar
  cp "$DEPLOY" target/java-tools/deploy.jar
  podman build -q -f tools/java-tests.Dockerfile -t nfm-java8 >/dev/null
  exec podman run --rm \
    -v "$PWD:/w:ro" \
    -v "$PWD/target:/w/target:rw" \
    nfm-java8 \
    bash -ec '
      Xvfb :99 -screen 0 800x600x24 >/dev/null 2>&1 &
      sleep 1
      mkdir -p /b
      javac -encoding UTF-8 -cp /w/target/java-tools/plugin.jar:/w/target/java-tools/deploy.jar -d /b /w/reference/vineflower/*.java /w/tests/java/*.java
      cd /w && DISPLAY=:99 java -Djava.awt.headless=false -cp "/b:/w/target/java-tools/plugin.jar:/w/target/java-tools/deploy.jar" Suite
    '
fi

mkdir -p target/java-tests
# -encoding UTF-8 is pinned on BOTH paths so string constants cannot
# drift between the mac and the container (the sources are pure ASCII
# today; the pin makes that irrelevant)
javac -encoding UTF-8 -cp "$PLUGIN" -d target/java-tests "$SRC_REF"/*.java "$SRC_TEST"/*.java

if [[ "${1:-}" == "--coverage" ]]; then
  # JaCoCo 0.8.12 against the compiled ORIGINAL classes - the metric
  # for "full java code coverage". Agent + cli jars download once into
  # target/java-tools/ (tooling, not game content - no provenance
  # implications).
  mkdir -p target/java-tools
  AGENT="target/java-tools/org.jacoco.agent-0.8.12-runtime.jar"
  CLI="target/java-tools/org.jacoco.cli-0.8.12-nodeps.jar"
  if [ ! -f "$AGENT" ]; then
    curl -fsSL -o "$AGENT" "https://repo1.maven.org/maven2/org/jacoco/org.jacoco.agent/0.8.12/org.jacoco.agent-0.8.12-runtime.jar"
  fi
  if [ ! -f "$CLI" ]; then
    curl -fsSL -o "$CLI" "https://repo1.maven.org/maven2/org/jacoco/org.jacoco.cli/0.8.12/org.jacoco.cli-0.8.12-nodeps.jar"
  fi
  java -Dapple.awt.UIElement=true -javaagent:"$AGENT"=destfile=target/java-tests/jacoco.exec -cp target/java-tests Suite
  java -jar "$CLI" report target/java-tests/jacoco.exec \
    --classfiles target/java-tests \
    --sourcefiles reference/vineflower \
    --html target/java-tests/coverage-html \
    --csv target/java-tests/coverage.csv \
    --name nfm-behavior >/dev/null
  echo "== per-class coverage: instruction% / branch% (missed/total) =="
  # csv rows are class-level with 13 columns
  awk -F, 'NR>1 && NF==13 {
    im[$3]+=$4; ic[$3]+=$5; bm[$3]+=$6; bc[$3]+=$7
  } END {
    for (c in im) {
      it=im[c]+ic[c]; bt=bm[c]+bc[c]
      printf "%5.1f%%  %5.1f%%  %s (inst %d/%d, branch %d/%d)\n",
        100*ic[c]/it, (bt ? 100*bc[c]/bt : 0), c, im[c], it, bm[c], bt
    }
  }' target/java-tests/coverage.csv | sort -k1 -n
  exit 0
fi

# no headless flag: the original code instantiates real Applets, which
# throws HeadlessException under a forced-headless toolkit; the mac AWT
# initializes fine without one (the BMP rigs proved it). The UIElement
# flag makes the JVM a background app on macOS - no Dock icon, no focus
# stealing from the user's terminal on every suite run (harmless no-op
# on Linux; the container runs under Xvfb anyway).
java -Dapple.awt.UIElement=true -cp target/java-tests Suite
