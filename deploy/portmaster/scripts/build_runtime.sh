#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_BUILD="$(cd "$HERE/../runtime-build" && pwd)"

command -v docker >/dev/null 2>&1 || {
    echo "ERROR: Docker is required to build the ARM64 runtime." >&2
    exit 1
}

cd "$RUNTIME_BUILD"
docker compose up --build --abort-on-container-exit --exit-code-from runtime_builder
