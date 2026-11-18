# The java behavior-test runner: JDK 8 + Xvfb (the original code
# requires headful AWT - Applet's constructor throws HeadlessException
# otherwise) + fonts for the draw paths. Public packages only; no repo
# content baked in. Built once by tools/java-tests.sh and cached.
# The base is DIGEST-PINNED to the exact image the committed
# baselines were generated on (podman images --digests); a floating
# tag would move the rasterizer/apt state and shift every pin loudly.
FROM docker.io/library/eclipse-temurin:8-jdk@sha256:c424f46b825d495a28e8f26ca5f411585119e5d9bd9152521180b9d5e5a303c4
RUN apt-get update \
 && apt-get install -y --no-install-recommends xvfb fontconfig fonts-dejavu-core \
      libxtst6 libxext6 libxrender1 libxi6 \
 && rm -rf /var/lib/apt/lists/*
