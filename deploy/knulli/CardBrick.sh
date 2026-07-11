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

# --------------------------------------------------------------- splash
# Mounting the runtime, the seed-db checks, and Python+pygame startup
# add up to several seconds of black screen. Paint a pre-rendered image
# (built by scripts/make_splash.py) straight into the framebuffer first
# thing — no interpreter needed, and SDL replaces it when the app
# presents its first frame. No matching image for this panel just means
# the screen stays black like before; never a failed launch.
show_splash() {
    [ -w /dev/fb0 ] || return 0
    local size bpp w h
    size="$(cat /sys/class/graphics/fb0/virtual_size 2>/dev/null)" || return 0
    bpp="$(cat /sys/class/graphics/fb0/bits_per_pixel 2>/dev/null)" || return 0
    w="${size%,*}"
    h="${size#*,}"
    # virtual_size is often double-height for page flipping; the visible
    # page (pan offset 0) is the panel itself.
    local candidate img
    for candidate in "$h" "$((h / 2))"; do
        img="${GAMEDIR}/splash/splash-${w}x${candidate}-${bpp}.raw.gz"
        if [ -f "$img" ]; then
            zcat "$img" > /dev/fb0 2>/dev/null
            return 0
        fi
    done
}
show_splash

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

# ------------------------------------------------------- seed deck(s)
# If the package was built with --deck (see PACKAGING.md), a database
# already containing the imported deck(s) ships at GAMEDIR/seed-data/.
# Install it when the device has no database yet, OR when the existing
# database contains zero cards — an empty db is what every pre-seed
# launch (even just the smoke test) leaves behind, and skipping in that
# case would strand the device on the "No cards imported" screen
# forever. A database with any cards in it is real progress and is
# never touched; the empty one we do replace is still backed up.
SEED_DB="${GAMEDIR}/seed-data/cardbrick.db"
SEED_MEDIA="${GAMEDIR}/seed-data/media"
DEVICE_DB="${CARD_BRICK_DATA_DIR}/cardbrick.db"
if [ -d "$SEED_MEDIA" ]; then
    export CARDBRICK_EXTRA_MEDIA_DIRS="${SEED_MEDIA}${CARDBRICK_EXTRA_MEDIA_DIRS:+:$CARDBRICK_EXTRA_MEDIA_DIRS}"
fi

db_card_count() {
    "$PYTHON" - "$1" <<'PYEOF'
import sqlite3
import sys

db = sys.argv[1]
try:
    con = sqlite3.connect("file:" + db + "?mode=ro", uri=True)
    ok = con.execute("PRAGMA integrity_check").fetchone()[0]
    if ok != "ok":
        print("CORRUPT:" + str(ok))
    else:
        count = con.execute("SELECT COUNT(*) FROM cards").fetchone()[0]
        print("OK:" + str(count))
except Exception as exc:
    print("ERROR:" + str(exc))
PYEOF
}

if [ -f "$SEED_DB" ]; then
    INSTALL_SEED=""
    BACKUP_EXISTING=""
    SEED_CHECK="$(db_card_count "$SEED_DB" 2>>"$LAUNCH_LOG")"
    if [ ! -f "$DEVICE_DB" ]; then
        INSTALL_SEED="first boot (no database yet)"
    else
        DEVICE_CHECK="$(db_card_count "$DEVICE_DB" 2>>"$LAUNCH_LOG")"
        case "$DEVICE_CHECK" in
            OK:0)
                INSTALL_SEED="existing database is empty (0 cards)"
                BACKUP_EXISTING="1"
                ;;
            OK:*)
                log "seed-data present but device database has ${DEVICE_CHECK#OK:} card(s) — leaving it alone"
                ;;
            *)
                INSTALL_SEED="existing database is unreadable (${DEVICE_CHECK})"
                BACKUP_EXISTING="1"
                ;;
        esac
    fi
    if [ -n "$INSTALL_SEED" ]; then
        case "$SEED_CHECK" in
            OK:0)
                log "WARN: seed-data database has 0 cards — not installing"
                ;;
            OK:*)
                SEED_TMP_DB="${DEVICE_DB}.seed-tmp-$$"
                rm -f "$SEED_TMP_DB"
                DB_BYTES="$(wc -c < "$SEED_DB" 2>/dev/null | tr -d ' ')"
                log "installing seeded deck(s): ${INSTALL_SEED}"
                log "copying seed database (${DB_BYTES:-unknown} bytes)"
                if cp "$SEED_DB" "$SEED_TMP_DB" 2>>"$LAUNCH_LOG"; then
                    TMP_CHECK="$(db_card_count "$SEED_TMP_DB" 2>>"$LAUNCH_LOG")"
                    if [ "$TMP_CHECK" = "$SEED_CHECK" ]; then
                        if [ -f "$SEED_TMP_DB" ]; then
                            if [ -n "$BACKUP_EXISTING" ] && [ -f "$DEVICE_DB" ]; then
                                STAMP="$(date +%Y%m%dT%H%M%S)"
                                if mv "$DEVICE_DB" "${DEVICE_DB}.pre-seed-${STAMP}" 2>>"$LAUNCH_LOG"; then
                                    log "backed up previous database as .pre-seed-${STAMP}"
                                else
                                    log "WARN: could not back up previous database — seed database not installed"
                                    rm -f "$SEED_TMP_DB"
                                fi
                                rm -f "${DEVICE_DB}-wal" "${DEVICE_DB}-shm"
                            fi
                        fi

                        if [ -f "$SEED_TMP_DB" ]; then
                            if mv "$SEED_TMP_DB" "$DEVICE_DB" 2>>"$LAUNCH_LOG"; then
                                log "seed install OK (${SEED_CHECK#OK:} card(s))"
                            else
                                log "WARN: seed install failed while moving database into place"
                                rm -f "$SEED_TMP_DB"
                            fi
                        fi
                    else
                        log "WARN: copied seed database failed verification (${TMP_CHECK})"
                        rm -f "$SEED_TMP_DB"
                    fi
                else
                    log "WARN: seed database copy failed — starting without seeded decks"
                    rm -f "$SEED_TMP_DB"
                fi
                ;;
            *)
                log "WARN: seed-data database is unreadable (${SEED_CHECK}) — not installing"
                ;;
        esac
    fi
fi

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
    echo "CARDBRICK_EXTRA_MEDIA_DIRS=${CARDBRICK_EXTRA_MEDIA_DIRS:-<unset>}"
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

# Sync is opt-in: the first `CardBrick.sh sync --name NAME` creates sync.json.
# Network failure never prevents study or changes the app's exit status.
if [ -f "${CARD_BRICK_DATA_DIR}/sync.json" ]; then
    "$PYTHON" main.py --knulli sync >> "$LAUNCH_LOG" 2>&1 \
        || log "WARN: pre-launch sync unavailable"
fi
"$PYTHON" main.py --knulli study --fullscreen >> "$LAUNCH_LOG" 2>&1
app_status=$?
if [ -f "${CARD_BRICK_DATA_DIR}/sync.json" ]; then
    "$PYTHON" main.py --knulli sync --backup-only >> "$LAUNCH_LOG" 2>&1 \
        || log "WARN: post-session backup unavailable"
fi
exit "$app_status"
