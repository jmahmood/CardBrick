#!/usr/bin/env bash
# Build the ARM64 pygame-ce squashfs runtime on a PC (host-side wrapper
# around runtime-build/docker-compose.yml).
#
# Output: deploy/knulli/runtime-build/build/pygame-ce_<PG>_python_<PY>.squashfs
#
# Requirements on the host: docker + docker compose. On an x86 host the
# linux/arm64 container needs binfmt/qemu emulation (see error hint).
#
# Already have a runtime squashfs (e.g. built earlier, or from the
# upstream pygame-ce-runtime project)? Skip this script and pass it to
# build_package.sh via RUNTIME_SQUASHFS=/path/to/file.squashfs.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${HERE}/../runtime-build"

command -v docker >/dev/null 2>&1 || {
    echo "ERROR: docker not found. Install Docker, or supply a prebuilt" >&2
    echo "runtime via RUNTIME_SQUASHFS= when running build_package.sh." >&2
    exit 1
}

# Fail early with a useful hint if arm64 emulation is missing.
if [ "$(uname -m)" != "aarch64" ] && [ "$(uname -m)" != "arm64" ]; then
    if ! docker run --rm --platform linux/arm64 python:3.12.8-bookworm true 2>/dev/null; then
        echo "ERROR: this host cannot run linux/arm64 containers." >&2
        echo "Enable qemu binfmt emulation and retry:" >&2
        echo "    docker run --privileged --rm tonistiigi/binfmt --install arm64" >&2
        exit 1
    fi
fi

# shellcheck disable=SC1091
. "${BUILD_DIR}/build-config"
ARTIFACT="pygame-ce_${PYGAME_VERSION}_python_${PYTHON_VERSION}.squashfs"

echo "Building ${ARTIFACT} (this compiles pygame-ce under emulation —"
echo "expect it to take a while on an x86 host)..."
( cd "$BUILD_DIR" && docker compose up --abort-on-container-exit )
( cd "$BUILD_DIR" && docker compose down --remove-orphans ) || true

OUT="${BUILD_DIR}/build/${ARTIFACT}"
if [ ! -s "$OUT" ]; then
    echo "ERROR: expected artifact not found: ${OUT}" >&2
    echo "Check the container output above for the real failure." >&2
    exit 1
fi

echo "OK: ${OUT} ($(du -h "$OUT" | cut -f1))"
[ -f "${BUILD_DIR}/build/runtime-manifest.txt" ] \
    && echo "Manifest: ${BUILD_DIR}/build/runtime-manifest.txt"
