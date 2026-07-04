#!/bin/sh
# CardBrick Spanish — Knulli / Batocera-style launch script (TEMPLATE).
#
# Copy this next to the app on the device and adapt the three PATH
# variables below. It is deliberately cautious: it never deletes
# anything, creates only the data directory, and logs everything it
# does so failures are debuggable from the SD card.
#
# Expected layout on the device (see package_layout.md):
#
#   /userdata/roms/ports/CardBrickSpanish/
#     launch_cardbrick_spanish.sh   <- this file (read-only ok)
#     cardbrick-py/                 <- the app (read-only ok)
#     runtime/                      <- OPTIONAL bundled ARM64 Python or
#                                      vendored site-packages (read-only)
#   /userdata/saves/cardbrick/      <- ALL mutable data (auto-created)

set -u  # undefined variables are bugs; do NOT set -e, we want to log

# ---------------------------------------------------------------- paths
APP_ROOT="$(dirname "$(readlink -f "$0")")"
APP_DIR="${APP_ROOT}/cardbrick-py"

# All mutable state lives OUTSIDE the app/runtime bundle. Respect an
# existing CARD_BRICK_DATA_DIR from the environment, else default to the
# Knulli userdata partition (writable, survives firmware upgrades).
CARD_BRICK_DATA_DIR="${CARD_BRICK_DATA_DIR:-/userdata/saves/cardbrick}"
export CARD_BRICK_DATA_DIR

LOG_DIR="${CARD_BRICK_DATA_DIR}/logs"
LAUNCH_LOG="${LOG_DIR}/launch.log"
mkdir -p "$LOG_DIR" || {
    echo "FATAL: cannot create ${LOG_DIR} — is /userdata writable?" >&2
    exit 1
}

# ------------------------------------------------------------- runtime
# Option A (preferred): a bundled portable ARM64 Python in ./runtime
# (e.g. from python-build-standalone or a PortMaster runtime), so the
# device needs nothing preinstalled. Uncomment and adapt:
#
# export PYTHONHOME="${APP_ROOT}/runtime"
# PYTHON="${APP_ROOT}/runtime/bin/python3"
#
# Option B: the firmware's own python3 plus dependencies vendored into
# ./vendor at packaging time on a PC (see deploy/knulli/README.md):
#
#   pip install --target vendor \
#       --platform manylinux2014_aarch64 --only-binary=:all: \
#       -r requirements-runtime.txt
#
PYTHON="${PYTHON:-python3}"
if [ -d "${APP_ROOT}/vendor" ]; then
    export PYTHONPATH="${APP_ROOT}/vendor${PYTHONPATH:+:$PYTHONPATH}"
fi

# ----------------------------------------------------------------- SDL
# Knulli builds usually pick the right KMS/DRM video driver on their
# own; only force one if the display stays black. Candidates seen on
# Batocera-derived firmwares: kmsdrm, mali, wayland, x11.
# export SDL_VIDEODRIVER=kmsdrm
# export SDL_AUDIODRIVER=alsa
#
# The app logs the driver it actually got — check cardbrick.log.

# -------------------------------------------------------------- launch
{
    echo "=== CardBrick Spanish launch $(date) ==="
    echo "APP_DIR=${APP_DIR}"
    echo "CARD_BRICK_DATA_DIR=${CARD_BRICK_DATA_DIR}"
    echo "PYTHON=${PYTHON} ($(${PYTHON} --version 2>&1))"
    echo "PYTHONHOME=${PYTHONHOME:-<unset>}"
    echo "PYTHONPATH=${PYTHONPATH:-<unset>}"
    echo "SDL_VIDEODRIVER=${SDL_VIDEODRIVER:-<auto>}"
    echo "SDL_AUDIODRIVER=${SDL_AUDIODRIVER:-<auto>}"
} >> "$LAUNCH_LOG" 2>&1

cd "$APP_DIR" || {
    echo "FATAL: app directory not found: ${APP_DIR}" >> "$LAUNCH_LOG"
    exit 1
}

# First run on a new device? Run the smoke test once and keep its
# output; it costs a second and saves an afternoon.
if [ ! -f "${CARD_BRICK_DATA_DIR}/.smoke-ok" ]; then
    if "$PYTHON" main.py --knulli --smoke-test >> "$LAUNCH_LOG" 2>&1; then
        touch "${CARD_BRICK_DATA_DIR}/.smoke-ok"
    else
        echo "smoke test FAILED — see above" >> "$LAUNCH_LOG"
        # Still try to start: the app shows its own error screens.
    fi
fi

exec "$PYTHON" main.py --knulli study --fullscreen \
    >> "$LAUNCH_LOG" 2>&1
