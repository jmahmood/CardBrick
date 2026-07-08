#!/usr/bin/env bash
# Assemble the deployable CardBrick package for Knulli / RG35XX SP.
#
#   deploy/knulli/dist/
#   ├── CardBrick.sh                 # goes to /userdata/roms/ports/
#   ├── CardBrick/                   # goes to /userdata/roms/ports/
#   │   ├── cardbrick-py/            # the app, verbatim (no tests/caches)
#   │   ├── runtime/pygame-ce_*.squashfs
#   │   ├── VERSION  BUILD_INFO  PACKAGE_MANIFEST.sha256
#   └── CardBrick-knulli-v<ver>.zip  # same content, zipped for SD card
#
# Usage:
#   scripts/build_package.sh                 # runtime from runtime-build/build/
#   RUNTIME_SQUASHFS=/path/x.squashfs scripts/build_package.sh
#   scripts/build_package.sh --no-runtime    # app-only (fast re-deploys of
#                                            # code onto a device that
#                                            # already has the runtime)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KNULLI_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${KNULLI_DIR}/../.." && pwd)"
APP_SRC="${REPO_ROOT}/cardbrick-py"
DIST="${KNULLI_DIR}/dist"
STAGE="${DIST}/CardBrick"

WITH_RUNTIME=1
for arg in "$@"; do
    case "$arg" in
        --no-runtime) WITH_RUNTIME=0 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

[ -f "${APP_SRC}/main.py" ] || { echo "ERROR: ${APP_SRC}/main.py missing" >&2; exit 1; }

VERSION="$(sed -n 's/^__version__ *= *"\([^"]*\)".*/\1/p' \
    "${APP_SRC}/cardbrick/__init__.py" | head -n 1)"
VERSION="${VERSION:-0.0.0}"

# ----------------------------------------------------------- find runtime
RUNTIME_FILE=""
if [ "$WITH_RUNTIME" -eq 1 ]; then
    if [ -n "${RUNTIME_SQUASHFS:-}" ]; then
        RUNTIME_FILE="$RUNTIME_SQUASHFS"
    else
        RUNTIME_FILE="$(ls -1t "${KNULLI_DIR}/runtime-build/build/"pygame-ce_*.squashfs 2>/dev/null | head -n 1 || true)"
    fi
    if [ -z "$RUNTIME_FILE" ] || [ ! -s "$RUNTIME_FILE" ]; then
        echo "ERROR: no runtime squashfs found." >&2
        echo "Build one first:   scripts/build_runtime.sh" >&2
        echo "or point at one:   RUNTIME_SQUASHFS=/path/to/pygame-ce_*.squashfs $0" >&2
        echo "or skip it:        $0 --no-runtime" >&2
        exit 1
    fi
fi

# ------------------------------------------------------------------ stage
echo "Staging CardBrick v${VERSION} -> ${STAGE}"
rm -rf "$STAGE" "${DIST}/CardBrick.sh"
mkdir -p "$STAGE"

# Copy the app verbatim, minus things the device never needs. tar keeps
# this portable (no rsync requirement on the host).
( cd "${REPO_ROOT}" && tar cf - \
      --exclude='cardbrick-py/tests' \
      --exclude='cardbrick-py/data' \
      --exclude='__pycache__' \
      --exclude='.pytest_cache' \
      --exclude='*.pyc' \
      --exclude='.DS_Store' \
      cardbrick-py ) | ( cd "$STAGE" && tar xf - )

install -m 755 "${KNULLI_DIR}/CardBrick.sh" "${DIST}/CardBrick.sh"

if [ -n "$RUNTIME_FILE" ]; then
    mkdir -p "${STAGE}/runtime"
    cp "$RUNTIME_FILE" "${STAGE}/runtime/"
    RUNTIME_MANIFEST="$(dirname "$RUNTIME_FILE")/runtime-manifest.txt"
    [ -f "$RUNTIME_MANIFEST" ] && cp "$RUNTIME_MANIFEST" "${STAGE}/runtime/"
fi

echo "$VERSION" > "${STAGE}/VERSION"
{
    echo "version=${VERSION}"
    echo "built=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "git=$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
    echo "runtime=$( [ -n "$RUNTIME_FILE" ] && basename "$RUNTIME_FILE" || echo none )"
} > "${STAGE}/BUILD_INFO"

# Checksums over everything in the bundle (verified by validate_package.sh
# and re-verifiable on-device over SSH).
( cd "$STAGE" && find . -type f ! -name PACKAGE_MANIFEST.sha256 -print0 \
      | sort -z | xargs -0 sha256sum ) > "${STAGE}/PACKAGE_MANIFEST.sha256"

# -------------------------------------------------------------------- zip
SUFFIX=""
[ "$WITH_RUNTIME" -eq 0 ] && SUFFIX="-noruntime"
ZIP_NAME="CardBrick-knulli-v${VERSION}${SUFFIX}.zip"
if command -v zip >/dev/null 2>&1; then
    ( cd "$DIST" && rm -f "$ZIP_NAME" && zip -qr "$ZIP_NAME" CardBrick.sh CardBrick )
    echo "Zip:   ${DIST}/${ZIP_NAME}"
else
    echo "WARN: 'zip' not found — skipped zip archive (dist/ tree is complete)"
fi

echo "Stage: ${STAGE}"
echo "Next:  scripts/validate_package.sh"
