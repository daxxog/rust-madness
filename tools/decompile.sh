#!/usr/bin/env bash
# Regenerate reference/ from mad.jar inside an ephemeral podman container.
#
#   tools/decompile.sh [--rebuild]
#
# --rebuild forces the tool image to rebuild (pick up new tool versions).
set -euo pipefail
cd "$(dirname "$0")/.."

IMAGE=localhost/nfm-decompilers

if [ "${1:-}" = "--rebuild" ]; then
  podman rmi -f "$IMAGE" >/dev/null 2>&1 || true
fi

podman build --quiet -t "$IMAGE" -f tools/Dockerfile tools

mkdir -p reference
exec podman run --rm -v "$PWD":/work:z -w /work "$IMAGE" \
  /usr/local/bin/container-decompile.sh
