# Ephemeral environment for decompiling mad.jar.
#
# The game classes are Java 1.1-era bytecode (class file format 45.748, i.e.
# major revision 45 with the non-JDK minor revision 748 of the original
# compiler), so we run several decompilers against it and cross-check their
# output. Everything is pinned; rebuilds are cache-friendly and reproducible.
#
# The host wrapper `tools/decompile.sh` builds this image and runs
# container-decompile.sh; you rarely need to touch this file.
FROM eclipse-temurin:21-jdk

ARG CFR_VERSION=0.152
ARG VINEFLOWER_VERSION=1.10.1
ARG PROCYON_VERSION=0.6.0

RUN apt-get update \
 && apt-get install -y --no-install-recommends curl unzip \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/tools
RUN curl -fsSL --retry 5 -o cfr.jar \
      "https://repo1.maven.org/maven2/org/benf/cfr/${CFR_VERSION}/cfr-${CFR_VERSION}.jar" \
 && curl -fsSL --retry 5 -o vineflower.jar \
      "https://repo1.maven.org/maven2/org/vineflower/vineflower/${VINEFLOWER_VERSION}/vineflower-${VINEFLOWER_VERSION}.jar" \
 && curl -fsSL --retry 5 -o procyon.jar \
      "https://repo1.maven.org/maven2/org/bitbucket/mstrobel/procyon-compilertools/${PROCYON_VERSION}/procyon-compilertools-${PROCYON_VERSION}.jar" \
 && curl -fsSL --retry 5 -o procyon-core.jar \
      "https://repo1.maven.org/maven2/org/bitbucket/mstrobel/procyon-core/${PROCYON_VERSION}/procyon-core-${PROCYON_VERSION}.jar"

ENV CFR_JAR=/opt/tools/cfr.jar \
    VINEFLOWER_JAR=/opt/tools/vineflower.jar \
    PROCYON_JAR=/opt/tools/procyon.jar \
    PROCYON_CORE_JAR=/opt/tools/procyon-core.jar \
    CFR_VERSION=${CFR_VERSION} \
    VINEFLOWER_VERSION=${VINEFLOWER_VERSION} \
    PROCYON_VERSION=${PROCYON_VERSION}

COPY container-decompile.sh container-check-compile.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/container-decompile.sh /usr/local/bin/container-check-compile.sh

# Procyon on Maven Central (procyon-compilertools) has no CLI driver, so we
# drive the library API with a tiny helper (see procyon-driver/PxDriver.java).
COPY procyon-driver/PxDriver.java /opt/tools/PxDriver.java
RUN javac -cp /opt/tools/procyon.jar /opt/tools/PxDriver.java

# API stubs for era-specific classes (sun.audio is a JDK 1.x internal,
# netscape.javascript is the Java plugin's liveconnect class) so the
# decompiled trees can round-trip through javac unchanged.
COPY javac-stubs /opt/tools/javac-stubs

WORKDIR /work
