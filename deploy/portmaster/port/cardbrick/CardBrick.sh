#!/usr/bin/env bash
# CardBrick-Py experimental PortMaster launcher.

XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
controlfolder=""
for folder in \
    /userdata/roms/ports/PortMaster \
    /userdata/system/.local/share/PortMaster \
    /opt/system/Tools/PortMaster \
    /opt/tools/PortMaster \
    "$XDG_DATA_HOME/PortMaster" \
    /roms/ports/PortMaster \
    /roms/ports/PortMaster/PortMaster; do
    if [ -f "$folder/control.txt" ]; then
        controlfolder="$folder"
        break
    fi
done

# CFWs place PortMaster in different mount roots. Keep discovery bounded and
# use it only when the standard locations above do not apply.
if [ -z "$controlfolder" ]; then
    candidate="$(find / -maxdepth 6 -type f -iname control.txt \
        -path '*[Pp]ort[Mm]aster*' -print -quit 2>/dev/null || true)"
    [ -z "$candidate" ] || controlfolder="$(dirname "$candidate")"
fi

if [ -z "$controlfolder" ] || [ ! -f "$controlfolder/control.txt" ]; then
    echo "CardBrick: PortMaster control.txt could not be found" >&2
    exit 1
fi

# Some older PortMaster control files rewrite controlfolder while they are
# sourced. Keep the directory we selected so their stale path cannot redirect
# the port onto another mount.
PORTMASTER_CONTROL_DIR="$controlfolder"

# PortMaster's control.txt is the compatibility API for CFW paths, device
# architecture, permissions, controls, and cleanup helpers.
source "$controlfolder/control.txt"
[ -n "${CFW_NAME:-}" ] && [ -f "$PORTMASTER_CONTROL_DIR/mod_${CFW_NAME}.txt" ] && \
    source "$PORTMASTER_CONTROL_DIR/mod_${CFW_NAME}.txt"
controlfolder="$PORTMASTER_CONTROL_DIR"
# PortMaster keeps display/CFW information in a companion file rather than
# exporting it from every control.txt revision.
[ -f "$PORTMASTER_CONTROL_DIR/device_info.txt" ] && \
    source "$PORTMASTER_CONTROL_DIR/device_info.txt"
controlfolder="$PORTMASTER_CONTROL_DIR"
SDLDBUSERFILE="$PORTMASTER_CONTROL_DIR/gamecontrollerdb.txt"
get_controls
controlfolder="$PORTMASTER_CONTROL_DIR"

# Older PortMaster releases do not export these values. Derive them from the
# running device and libc so the same launcher remains usable across releases.
if [ -z "${DEVICE_ARCH:-}" ]; then
    case "$(uname -m 2>/dev/null || true)" in
        aarch64|arm64) DEVICE_ARCH="aarch64" ;;
        *) DEVICE_ARCH="$(uname -m 2>/dev/null || true)" ;;
    esac
fi
if [ -z "${CFW_GLIBC:-}" ]; then
    glibc_version="$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}' || true)"
    case "$glibc_version" in
        [0-9]*.[0-9]*) CFW_GLIBC="$(printf '%s' "$glibc_version" | sed -E 's/^([0-9]+)\.([0-9]+).*$/\1\2/')" ;;
    esac
fi
SDL_GAMECONTROLLERCONFIG_FILE="$PORTMASTER_CONTROL_DIR/gamecontrollerdb.txt"
GPTOKEYB="$ESUDO $PORTMASTER_CONTROL_DIR/gptokeyb $ESUDOKILL"

# PortMaster's control and device-info scripts intentionally populate their
# variables incrementally, so enable unset-variable checking only afterwards.
set -u

case "$controlfolder" in
    /userdata/roms/ports/PortMaster)
        directory="userdata/roms"
        ;;
esac

echo "PortMaster control=$controlfolder/control.txt"
echo "PortMaster directory=$directory"

LAUNCHER_NAME="$(basename "$0")"
DEFAULT_PORT_NAME="${LAUNCHER_NAME%.sh}"
DEFAULT_PORT_NAME="$(printf '%s' "$DEFAULT_PORT_NAME" | tr '[:upper:]' '[:lower:]')"
PORT_NAME="${CARDBRICK_PORT_NAME:-$DEFAULT_PORT_NAME}"
GAMEDIR="/${directory}/ports/${PORT_NAME}"
CONFDIR="$GAMEDIR/conf"
APPDIR="$GAMEDIR/cardbrick-py"
RUNTIMEDIR="$GAMEDIR/runtime"
PYTHON="$RUNTIMEDIR/bin/python3"
LOGFILE="$GAMEDIR/log.txt"

mkdir -p "$CONFDIR"
cd "$GAMEDIR" || exit 1
exec > >(tee "$LOGFILE") 2>&1

cleanup() {
    if command -v pm_finish >/dev/null 2>&1; then
        pm_finish
    fi
}
trap cleanup EXIT INT TERM

fail() {
    if command -v pm_message >/dev/null 2>&1; then
        pm_message "CardBrick: $1"
    fi
    echo "FATAL: $1" >&2
    exit 1
}

if [ "${DEVICE_ARCH:-}" != "aarch64" ]; then
    fail "this package requires aarch64 (detected ${DEVICE_ARCH:-unknown})"
fi

# device_info.txt reports this as a compact value such as 231 or 233.
case "${CFW_GLIBC:-}" in
    ''|*[!0-9]*)
        fail "glibc version could not be determined; glibc 2.31 or newer is required"
        ;;
esac
if [ "$CFW_GLIBC" -lt 231 ]; then
    fail "glibc 2.31 or newer is required (detected ${CFW_GLIBC:-unknown})"
fi

case "${DISPLAY_WIDTH:-}" in ''|*[!0-9]*) fail "display width could not be determined" ;; esac
case "${DISPLAY_HEIGHT:-}" in ''|*[!0-9]*) fail "display height could not be determined" ;; esac
if [ "$DISPLAY_WIDTH" -lt 640 ] || [ "$DISPLAY_HEIGHT" -lt 480 ]; then
    fail "a 640x480-or-larger display is required (detected ${DISPLAY_WIDTH:-unknown}x${DISPLAY_HEIGHT:-unknown})"
fi

[ -d "$APPDIR" ] || fail "CardBrick application files are missing"
[ -f "$PYTHON" ] || fail "bundled Python runtime is missing"
[ -f "$APPDIR/main.py" ] || fail "CardBrick entry point is missing"

# FAT/exFAT installs can lose executable bits. PortMaster provides ESUDO for
# the few platforms where this requires elevated permissions.
$ESUDO chmod +x "$PYTHON" 2>/dev/null || true

export CARD_BRICK_DATA_DIR="$CONFDIR"
export CARDBRICK_MODE=knulli
export XDG_DATA_HOME="$CONFDIR"
export SDL_GAMECONTROLLERCONFIG="$sdl_controllerconfig"
export PYTHONHOME="$RUNTIMEDIR"
PY_LIB=""
for candidate in "$RUNTIMEDIR"/lib/python3.*; do
    if [ -d "$candidate" ]; then
        PY_LIB="$candidate"
        break
    fi
done
[ -n "$PY_LIB" ] || fail "bundled Python standard library is missing"
export PYTHONPATH="$PY_LIB:$PY_LIB/site-packages:$APPDIR"
export LD_LIBRARY_PATH="$RUNTIMEDIR/lib:$GAMEDIR/libs.aarch64${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PYTHONPYCACHEPREFIX="$CONFDIR/.pycache"

# Install a packaged deck only for a fresh or empty local database. Real study
# progress is never replaced; an empty/corrupt database is retained as backup.
SEED_DB="$GAMEDIR/seed-data/cardbrick.db"
SEED_MEDIA="$GAMEDIR/seed-data/media"
DEVICE_DB="$CONFDIR/cardbrick.db"
if [ -d "$SEED_MEDIA" ]; then
    export CARDBRICK_EXTRA_MEDIA_DIRS="$SEED_MEDIA${CARDBRICK_EXTRA_MEDIA_DIRS:+:$CARDBRICK_EXTRA_MEDIA_DIRS}"
fi

db_card_count() {
    "$PYTHON" - "$1" <<'PY'
import sqlite3
import sys

try:
    con = sqlite3.connect("file:" + sys.argv[1] + "?mode=ro", uri=True)
    integrity = con.execute("PRAGMA integrity_check").fetchone()[0]
    if integrity != "ok":
        print("CORRUPT:" + str(integrity))
    else:
        print("OK:" + str(con.execute("SELECT COUNT(*) FROM cards").fetchone()[0]))
except Exception as exc:
    print("ERROR:" + str(exc))
PY
}

if [ -f "$SEED_DB" ]; then
    seed_check="$(db_card_count "$SEED_DB")"
    install_seed=""
    backup_existing=""
    if [ ! -f "$DEVICE_DB" ]; then
        install_seed="first launch"
    else
        device_check="$(db_card_count "$DEVICE_DB")"
        case "$device_check" in
            OK:0) install_seed="existing database is empty"; backup_existing=1 ;;
            OK:*) echo "seed deck present; keeping existing ${device_check#OK:}-card database" ;;
            *) install_seed="existing database is unreadable"; backup_existing=1 ;;
        esac
    fi

    if [ -n "$install_seed" ]; then
        case "$seed_check" in
            OK:0) echo "seed deck has no cards; not installing" ;;
            OK:*)
                seed_tmp="${DEVICE_DB}.seed-tmp-$$"
                rm -f "$seed_tmp"
                if cp "$SEED_DB" "$seed_tmp" && \
                   [ "$(db_card_count "$seed_tmp")" = "$seed_check" ]; then
                    if [ -n "$backup_existing" ] && [ -f "$DEVICE_DB" ]; then
                        stamp="$(date +%Y%m%dT%H%M%S)"
                        mv "$DEVICE_DB" "${DEVICE_DB}.pre-seed-${stamp}" || {
                            echo "could not back up existing database; not installing seed deck"
                            rm -f "$seed_tmp"
                        }
                        rm -f "${DEVICE_DB}-wal" "${DEVICE_DB}-shm"
                    fi
                    if [ -f "$seed_tmp" ] && mv "$seed_tmp" "$DEVICE_DB"; then
                        echo "seed deck installed (${seed_check#OK:} cards; $install_seed)"
                    fi
                else
                    echo "seed deck copy failed verification; not installing"
                    rm -f "$seed_tmp"
                fi
                ;;
            *) echo "seed deck is unreadable; not installing" ;;
        esac
    fi
fi

echo "CardBrick PortMaster launch"
echo "GAMEDIR=$GAMEDIR"
echo "DEVICE_ARCH=${DEVICE_ARCH:-unknown} CFW=${CFW_NAME:-unknown}"
echo "CFW_GLIBC=${CFW_GLIBC:-unknown}"
echo "DISPLAY=${DISPLAY_WIDTH:-unknown}x${DISPLAY_HEIGHT:-unknown}"
echo "PYTHON=$PYTHON"
echo "SDL_VIDEODRIVER=${SDL_VIDEODRIVER:-auto}"
echo "SDL_AUDIODRIVER=${SDL_AUDIODRIVER:-auto}"

cd "$APPDIR" || fail "cannot enter CardBrick application directory"
$GPTOKEYB "$PYTHON" &
if command -v pm_platform_helper >/dev/null 2>&1; then
    pm_platform_helper "$PYTHON"
fi

if [ "$#" -gt 0 ]; then
    "$PYTHON" main.py --knulli "$@"
else
    "$PYTHON" main.py --knulli study --fullscreen
fi
