#!/bin/bash
# CardBrick — Knulli / RG35XX SP launch script (pygame-ce squashfs runtime).
#
# Lives at /userdata/roms/ports/CardBrick.sh next to the CardBrick/
# folder (see PACKAGING.md). Knulli's Ports menu runs it as root.
# Runtime model derived from https://github.com/viniciusfs/pygame-ce-runtime:
# a self-contained ARM64 Python venv in a squashfs image is loop-mounted,
# and the app runs on that interpreter. pygame-ce inside the runtime is
# linked against the FIRMWARE's SDL2, so Knulli's mali/kmsdrm video
# drivers work.
#
# Usage (also over SSH):
#   ./CardBrick.sh                    # normal launch: study, fullscreen
#   ./CardBrick.sh --smoke-test       # non-interactive deployment check
#   ./CardBrick.sh --input-diagnostic # controller test & calibration
#   ./CardBrick.sh <any main.py args> # passthrough
#
# The app/runtime tree is treated as read-only; ALL mutable state goes
# to $CARD_BRICK_DATA_DIR (default /userdata/saves/cardbrick).

set -u  # undefined vars are bugs; no set -e — we want to log failures

SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
GAMEDIR="${SCRIPT_DIR}/CardBrick"
APP_DIR="${GAMEDIR}/cardbrick-py"

# ------------------------------------------------------------- data dir
CARD_BRICK_DATA_DIR="${CARD_BRICK_DATA_DIR:-/userdata/saves/cardbrick}"
export CARD_BRICK_DATA_DIR
LOG_DIR="${CARD_BRICK_DATA_DIR}/logs"
LAUNCH_LOG="${LOG_DIR}/launch.log"
mkdir -p "$LOG_DIR" || {
    echo "FATAL: cannot create ${LOG_DIR} — is /userdata writable?" >&2
    exit 1
}

log() { echo "$*" >> "$LAUNCH_LOG"; }
fail() {
    log "FATAL: $*"
    echo "FATAL: $*" >&2
    exit 1
}

# ------------------------------------------------------- seed deck(s)
# If the package was built with --deck (see PACKAGING.md), a database
# already containing the imported deck(s) ships at GAMEDIR/seed-data/.
# Install it on this device's very first boot only — i.e. only if no
# database exists yet in the data dir — so a later package update never
# clobbers review progress that has since accumulated on-device.
SEED_DIR="${GAMEDIR}/seed-data"
if [ -f "${SEED_DIR}/cardbrick.db" ] && [ ! -f "${CARD_BRICK_DATA_DIR}/cardbrick.db" ]; then
    log "first boot: installing seeded deck(s) from ${SEED_DIR}"
    if cp "${SEED_DIR}/cardbrick.db" "${CARD_BRICK_DATA_DIR}/cardbrick.db" 2>>"$LAUNCH_LOG"; then
        if [ -d "${SEED_DIR}/media" ]; then
            mkdir -p "${CARD_BRICK_DATA_DIR}/media"
            cp -R "${SEED_DIR}/media/." "${CARD_BRICK_DATA_DIR}/media/" 2>>"$LAUNCH_LOG"
        fi
        log "seed install OK"
    else
        log "WARN: seed install failed — starting with an empty database instead"
    fi
fi

log "=== CardBrick launch $(date) ==="

[ -d "$APP_DIR" ] || fail "app directory not found: $APP_DIR"

# -------------------------------------------------------------- runtime
# Newest squashfs wins, so a runtime upgrade is just one file copy.
RUNTIME_SQUASHFS="$(ls -1t "${GAMEDIR}/runtime/"pygame-ce_*.squashfs 2>/dev/null | head -n 1)"
RUNTIME_EXTRACTED="${GAMEDIR}/runtime/extracted"   # unsquashed fallback
MOUNTPOINT="${CARDBRICK_RUNTIME_MOUNT:-/tmp/cardbrick-runtime}"

MOUNTED=""   # how we mounted, for cleanup: "", "loop" or "fuse"
RUNTIME_DIR=""

cleanup() {
    cd /
    case "$MOUNTED" in
        loop) umount "$MOUNTPOINT" 2>/dev/null ;;
        fuse) fusermount -u "$MOUNTPOINT" 2>/dev/null \
                  || umount "$MOUNTPOINT" 2>/dev/null ;;
    esac
}
trap cleanup EXIT INT TERM

if [ -n "$RUNTIME_SQUASHFS" ]; then
    mkdir -p "$MOUNTPOINT"
    # A stale mount from a crashed run would shadow the new runtime.
    umount "$MOUNTPOINT" 2>/dev/null
    if mount -o loop,ro "$RUNTIME_SQUASHFS" "$MOUNTPOINT" 2>>"$LAUNCH_LOG"; then
        MOUNTED="loop"
        RUNTIME_DIR="$MOUNTPOINT"
    elif command -v squashfuse >/dev/null 2>&1 \
            && squashfuse "$RUNTIME_SQUASHFS" "$MOUNTPOINT" 2>>"$LAUNCH_LOG"; then
        MOUNTED="fuse"
        RUNTIME_DIR="$MOUNTPOINT"
    else
        log "WARN: could not mount $RUNTIME_SQUASHFS"
    fi
fi

if [ -z "$RUNTIME_DIR" ] && [ -x "${RUNTIME_EXTRACTED}/bin/python3" ]; then
    log "using extracted runtime fallback: $RUNTIME_EXTRACTED"
    RUNTIME_DIR="$RUNTIME_EXTRACTED"
fi

if [ -n "$RUNTIME_DIR" ]; then
    PYTHON="${RUNTIME_DIR}/bin/python3"
    [ -x "$PYTHON" ] || fail "runtime mounted but ${PYTHON} is missing/not executable"
    # stdlib dir, e.g. .../lib/python3.12 (version-agnostic glob)
    PY_LIB="$(ls -d "${RUNTIME_DIR}/lib/"python3.* 2>/dev/null | head -n 1)"
    [ -n "$PY_LIB" ] || fail "no lib/python3.* inside runtime ${RUNTIME_DIR}"
    export PYTHONHOME="$RUNTIME_DIR"
    export PYTHONPATH="${PY_LIB}:${PY_LIB}/site-packages"
    # libpython3.X.so.1.0 lives in runtime/lib (venv python is a copy of
    # a --enable-shared build)
    export LD_LIBRARY_PATH="${RUNTIME_DIR}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
else
    # Last resort: firmware python3 (needs pygame available system-wide).
    log "WARN: no usable bundled runtime; falling back to system python3"
    PYTHON="python3"
    command -v python3 >/dev/null 2>&1 || fail "no bundled runtime and no system python3"
fi

# The squashfs (and possibly the whole SD card) is read-only — keep
# .pyc files out of the app tree.
export PYTHONPYCACHEPREFIX="${CARD_BRICK_DATA_DIR}/.pycache"

# ------------------------------------------------------------------ SDL
# `mali` is what the upstream runtime validated on Knulli / Allwinner
# H700 (RG35XX family). If the screen stays black try kmsdrm:
#   SDL_VIDEODRIVER=kmsdrm ./CardBrick.sh
export SDL_VIDEODRIVER="${SDL_VIDEODRIVER:-mali}"
export SDL_AUDIODRIVER="${SDL_AUDIODRIVER:-alsa}"

# ---------------------------------------------------------------- launch
{
    echo "GAMEDIR=${GAMEDIR}"
    echo "RUNTIME_SQUASHFS=${RUNTIME_SQUASHFS:-<none>}"
    echo "RUNTIME_DIR=${RUNTIME_DIR:-<system python>}"
    echo "PYTHON=${PYTHON}"
    echo "PYTHONHOME=${PYTHONHOME:-<unset>}"
    echo "PYTHONPATH=${PYTHONPATH:-<unset>}"
    echo "SDL_VIDEODRIVER=${SDL_VIDEODRIVER}"
    echo "SDL_AUDIODRIVER=${SDL_AUDIODRIVER}"
    echo "CARD_BRICK_DATA_DIR=${CARD_BRICK_DATA_DIR}"
    "$PYTHON" --version 2>&1
} >> "$LAUNCH_LOG"

cd "$APP_DIR" || fail "cannot cd to ${APP_DIR}"

# First boot on this device: run the smoke test once and keep its output.
if [ ! -f "${CARD_BRICK_DATA_DIR}/.smoke-ok" ] && [ "$#" -eq 0 ]; then
    if "$PYTHON" main.py --knulli --smoke-test >> "$LAUNCH_LOG" 2>&1; then
        touch "${CARD_BRICK_DATA_DIR}/.smoke-ok"
        log "first-run smoke test PASSED"
    else
        log "first-run smoke test FAILED — see above; starting anyway"
    fi
fi

if [ "$#" -gt 0 ]; then
    # Passthrough mode (e.g. --smoke-test over SSH). Show output on the
    # terminal AND keep it in the log.
    "$PYTHON" main.py --knulli "$@" 2>&1 | tee -a "$LAUNCH_LOG"
    exit_code=${PIPESTATUS[0]}
    exit "$exit_code"
fi

"$PYTHON" main.py --knulli study --fullscreen >> "$LAUNCH_LOG" 2>&1
