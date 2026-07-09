#!/usr/bin/env bash
# Copy the built CardBrick package onto a Knulli SD card mounted on this
# PC. Works with either the single-card layout (SHARE partition with
# roms/ at its root) or a directly-mounted roms partition.
#
# Usage:
#   scripts/deploy_sdcard.sh                # auto-detect mounted card
#   scripts/deploy_sdcard.sh /media/me/SHARE
#   scripts/deploy_sdcard.sh /Volumes/SHARE --clean
#
# --clean removes the existing CardBrick/ folder on the card first
# (never touches anything else, and never touches /userdata/saves data
# on the device — that lives on the card only as saves/, which this
# script does not write to).

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST="$(cd "${HERE}/.." && pwd)/dist"
STAGE="${DIST}/CardBrick"

SD_ROOT=""
CLEAN=0
for arg in "$@"; do
    case "$arg" in
        --clean) CLEAN=1 ;;
        -*) echo "unknown option: $arg" >&2; exit 2 ;;
        *) SD_ROOT="$arg" ;;
    esac
done

[ -d "$STAGE" ] && [ -f "${DIST}/CardBrick.sh" ] || {
    echo "ERROR: nothing staged in ${DIST} — run scripts/build_package.sh first" >&2
    exit 1
}

# ------------------------------------------------- locate the ports dir
ports_dir_for() {
    # Accept the SHARE root, a roms dir, or the ports dir itself.
    for c in "$1/roms/ports" "$1/ports" "$1"; do
        case "$c" in */ports) [ -d "$c" ] && { echo "$c"; return 0; } ;; esac
    done
    return 1
}

PORTS_DIR=""
if [ -n "$SD_ROOT" ]; then
    PORTS_DIR="$(ports_dir_for "$SD_ROOT")" || {
        echo "ERROR: no roms/ports directory under ${SD_ROOT}." >&2
        echo "Is this the Knulli SHARE partition? (It should contain roms/, bios/, saves/...)" >&2
        exit 1
    }
else
    echo "No SD path given — scanning common mount points..."
    for base in /media/*/* /media/* /run/media/*/* /Volumes/*; do
        [ -d "$base" ] || continue
        if p="$(ports_dir_for "$base" 2>/dev/null)"; then
            echo "  found: $p"
            if [ -n "$PORTS_DIR" ]; then
                echo "ERROR: multiple candidate cards found — pass the path explicitly." >&2
                exit 1
            fi
            PORTS_DIR="$p"
        fi
    done
    [ -n "$PORTS_DIR" ] || {
        echo "ERROR: no mounted SD card with a roms/ports directory found." >&2
        echo "Mount the Knulli SHARE partition and pass its path, e.g.:" >&2
        echo "    $0 /media/\$USER/SHARE" >&2
        exit 1
    }
fi

echo "Deploying to: ${PORTS_DIR}"

if [ "$CLEAN" -eq 1 ] && [ -d "${PORTS_DIR}/CardBrick" ]; then
    echo "Removing existing ${PORTS_DIR}/CardBrick (--clean)"
    rm -rf "${PORTS_DIR}/CardBrick"
fi

# rsync when available (delta copy — the ~xx MB runtime is skipped when
# unchanged); plain cp otherwise.
if command -v rsync >/dev/null 2>&1; then
    rsync -rt --delete "${STAGE}/" "${PORTS_DIR}/CardBrick/"
    rsync -t --chmod=ugo+x "${DIST}/CardBrick.sh" "${PORTS_DIR}/CardBrick.sh"
else
    mkdir -p "${PORTS_DIR}/CardBrick"
    cp -R "${STAGE}/." "${PORTS_DIR}/CardBrick/"
    cp "${DIST}/CardBrick.sh" "${PORTS_DIR}/CardBrick.sh"
fi
chmod +x "${PORTS_DIR}/CardBrick.sh" 2>/dev/null || true  # FAT has no exec bit; Knulli copes

# Give the Ports entry a description and image: merge the metadata from
# assets/gameinfo.xml (repo root) into the card's gamelist.xml, keeping
# ES-owned play stats. Best-effort — never fails the deploy.
REPO_ROOT="$(cd "${HERE}/../../.." && pwd)"
GAMEINFO="${REPO_ROOT}/assets/gameinfo.xml"
if [ -f "$GAMEINFO" ] && command -v python3 >/dev/null 2>&1; then
    echo "Updating Ports gamelist metadata..."
    GL_NEW="$(mktemp)"
    if python3 "${HERE}/merge_gamelist.py" "$GAMEINFO" \
            "${PORTS_DIR}/gamelist.xml" > "$GL_NEW"; then
        cp "$GL_NEW" "${PORTS_DIR}/gamelist.xml"
    else
        echo "WARNING: gamelist merge failed — Ports entry will lack desc/image" >&2
    fi
    rm -f "$GL_NEW"
fi

echo "Flushing writes to the card (sync)..."
sync

echo
echo "Done. Now:"
echo "  1. Eject the card safely, put it in the device, boot."
echo "  2. If 'CardBrick' does not appear under Ports: Start menu ->"
echo "     Games Settings -> Update Gamelists (or just reboot)."
echo "  3. First launch runs a self-check; see"
echo "     /userdata/saves/cardbrick/logs/launch.log if anything is off."
