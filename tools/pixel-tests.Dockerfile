# The pixel-test runner: the SAME JDK 8 + Xvfb + fonts base as the java
# behavior runner (the original code requires headful AWT - Applet's
# constructor throws HeadlessException otherwise), plus the X11 libraries
# the pixel pins render through. Public packages only; no repo content
# baked in - the shipped text font rides the /w mount from tools/fonts/.
# Built once by tools/pixel-tests.sh and cached.
# The base is DIGEST-PINNED to the exact image the committed baselines
# (the glyph atlas and every pixel md5) were generated on (podman images
# --digests); a floating tag would move the rasterizer/apt state and
# shift every pin loudly.
FROM docker.io/library/eclipse-temurin:8-jdk@sha256:c424f46b825d495a28e8f26ca5f411585119e5d9bd9152521180b9d5e5a303c4
RUN apt-get update \
 && apt-get install -y --no-install-recommends xvfb fontconfig fonts-dejavu-core \
      libxtst6 libxext6 libxrender1 libxi6 \
 && rm -rf /var/lib/apt/lists/*
