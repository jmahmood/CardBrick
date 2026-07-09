#!/usr/bin/env bash
# Push the cardbrick-py Python code straight to a device over scp —
# no packaging, no runtime rebuild, no make. For fast iteration when
# only files under cardbrick-py/ changed (the usual case).
#
# Serves both deployment flavours, which share the same on-device
# layout (<install dir>/CardBrick/cardbrick-py/ beside CardBrick.sh):
#
#   Knulli / RG35XX SP:    /userdata/roms/ports/CardBrick
#   MinUI / TrimUI Brick:  wherever the package folder was copied on
#                          the SD card (auto-detected)
#
# Usage:
#   scripts/push_python.sh                            # CARDBRICK_DEVICE or root@knulli.local
#   scripts/push_python.sh --host root@192.168.1.42
#   scripts/push_python.sh --dest /mnt/SDCARD/Ports/CardBrick
#                                                     # skip auto-detection
#
# Env:
#   CARDBRICK_DEVICE   default host (e.g. root@192.168.1.42)
#   SSHPASS            used automatically when sshpass is installed
#
# Only code is copied: main.py and cardbrick/*.py. Assets, decks, the
# runtime, and all on-device data are untouched — for anything more
# than a code change, use the full pipeline in deploy/<platform>/.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_SRC="$(cd "${HERE}/.." && pwd)/cardbrick-py"

HOST="${CARDBRICK_DEVICE:-root@knulli.local}"
DEST=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --host) HOST="$2"; shift ;;
        --dest) DEST="$2"; shift ;;
        -h|--help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown option: $1 (try --help)" >&2; exit 2 ;;
    esac
    shift
done

[ -f "${APP_SRC}/main.py" ] || {
    echo "ERROR: ${APP_SRC}/main.py not found — run from a CardBrick checkout" >&2
    exit 1
}

SSH="ssh"
SCP="scp"
if [ -n "${SSHPASS:-}" ] && command -v sshpass >/dev/null 2>&1; then
    SSH="sshpass -e ssh"
    SCP="sshpass -e scp"
fi

# OpenSSH >= 9.0 speaks SFTP by default, which MinUI-style dropbear
# devices without an sftp subsystem reject; -O forces the classic scp
# protocol. Older scp clients don't know -O and don't need it.
SCP_FLAGS=()
if ! scp -O 2>&1 | grep -q "unknown option"; then
    SCP_FLAGS=(-O)
fi

run_remote() { $SSH "$HOST" "$@"; }

echo "Checking connection to ${HOST}..."
run_remote "true" || {
    echo "ERROR: cannot SSH to ${HOST}." >&2
    echo "  - Is the device on and on the network?" >&2
    echo "  - Try: scripts/push_python.sh --host root@<ip>" >&2
    exit 1
}

if [ -n "$DEST" ]; then
    run_remote "test -f '${DEST}/cardbrick-py/main.py'" || {
        echo "ERROR: no installed package at ${DEST} (expected ${DEST}/cardbrick-py/main.py)." >&2
        echo "Deploy the full package once first (deploy/<platform>/), then use this for updates." >&2
        exit 1
    }
else
    echo "Locating the installed package on the device..."
    DEST="$($SSH "$HOST" sh -s <<'REMOTE_EOF'
# Fixed locations first (Knulli), then a bounded search of the usual
# roots (MinUI SD cards put the folder wherever the user dropped it).
for d in /userdata/roms/ports/CardBrick /userdata/roms/ports/CardBrickSpanish; do
    [ -f "$d/cardbrick-py/main.py" ] && { echo "$d"; exit 0; }
done
for root in /mnt/SDCARD /userdata/roms; do
    [ -d "$root" ] || continue
    hit="$(find "$root" -maxdepth 5 -type f -name main.py 2>/dev/null \
        | grep '/cardbrick-py/main\.py$' | head -n 1)"
    [ -n "$hit" ] && { echo "${hit%/cardbrick-py/main.py}"; exit 0; }
done
exit 1
REMOTE_EOF
)" || {
        echo "ERROR: no installed CardBrick package found on the device." >&2
        echo "Deploy the full package once first (deploy/<platform>/), then use this for updates." >&2
        echo "If it lives somewhere unusual: scripts/push_python.sh --dest <install dir>" >&2
        exit 1
    }
    echo "  found: ${DEST}"
fi

echo "Copying Python code to ${HOST}:${DEST}/cardbrick-py ..."
$SCP ${SCP_FLAGS[@]+"${SCP_FLAGS[@]}"} -q \
    "${APP_SRC}/main.py" \
    "${HOST}:${DEST}/cardbrick-py/"
$SCP ${SCP_FLAGS[@]+"${SCP_FLAGS[@]}"} -q \
    "${APP_SRC}/cardbrick/"*.py \
    "${HOST}:${DEST}/cardbrick-py/cardbrick/"

# Stale bytecode is harmless (CPython revalidates against the source),
# but clearing it keeps the on-device tree clean and rules it out when
# debugging.
run_remote "rm -rf '${DEST}/cardbrick-py/cardbrick/__pycache__'"

echo "Done. Relaunch CardBrick on the device to pick up the new code."
