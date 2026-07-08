#!/usr/bin/env bash
# Copy the contents of deploy/knulli/dist onto a mounted Knulli SHARE
# partition's ports directory.
#
# Defaults match the usual macOS mount point:
#   scripts/copy_dist_to_ports.sh
#
# Optional overrides:
#   scripts/copy_dist_to_ports.sh /path/to/dist /Volumes/SHARE/roms/ports
#
# The transfer uses rsync when available because it is incremental and
# avoids rewriting unchanged files, which matters on SD cards. MP3 and
# JPEG files get a special copy-only-if-missing pass because they are
# usually the slowest files to rewrite.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_SOURCE="$(cd "${HERE}/.." && pwd)/dist"
DEFAULT_TARGET="/Volumes/SHARE/roms/ports"

SOURCE_DIR="${1:-$DEFAULT_SOURCE}"
TARGET_DIR="${2:-$DEFAULT_TARGET}"

usage() {
    cat <<EOF
Usage:
  $(basename "$0") [source-dist-dir] [target-ports-dir]

Defaults:
  source-dist-dir  ${DEFAULT_SOURCE}
  target-ports-dir ${DEFAULT_TARGET}
EOF
}

case "${1:-}" in
    -h|--help)
        usage
        exit 0
        ;;
esac

if [ ! -d "$SOURCE_DIR" ]; then
    echo "ERROR: source directory does not exist: ${SOURCE_DIR}" >&2
    echo "Build the Knulli package first, for example: cd deploy/knulli && make package" >&2
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    echo "ERROR: target directory does not exist: ${TARGET_DIR}" >&2
    echo "Mount the SHARE partition, then confirm it has roms/ports." >&2
    exit 1
fi

echo "Copying:"
echo "  from: ${SOURCE_DIR}/"
echo "  to:   ${TARGET_DIR}/"

if command -v rsync >/dev/null 2>&1; then
    # Flags are chosen for mounted FAT/exFAT-style SD cards:
    # - recursive copy with timestamps for incremental comparisons
    # - no owner/group/perms writes, which those filesystems often ignore
    # - FAT timestamp tolerance to avoid needless future recopies
    # - skip macOS metadata files
    # - copy new MP3/JPEG files, but never rewrite those files already on the card
    # - partial progress is useful when large runtime archives are present
    rsync \
        --recursive \
        --times \
        --links \
        --human-readable \
        --progress \
        --modify-window=2 \
        --no-owner \
        --no-group \
        --no-perms \
        --exclude='.DS_Store' \
        --exclude='._*' \
        --exclude='*.mp3' \
        --exclude='*.MP3' \
        --exclude='*.jpg' \
        --exclude='*.JPG' \
        --exclude='*.jpeg' \
        --exclude='*.JPEG' \
        "${SOURCE_DIR}/" \
        "${TARGET_DIR}/"

    rsync \
        --recursive \
        --times \
        --human-readable \
        --progress \
        --modify-window=2 \
        --no-owner \
        --no-group \
        --no-perms \
        --ignore-existing \
        --include='*/' \
        --include='*.mp3' \
        --include='*.MP3' \
        --include='*.jpg' \
        --include='*.JPG' \
        --include='*.jpeg' \
        --include='*.JPEG' \
        --exclude='*' \
        "${SOURCE_DIR}/" \
        "${TARGET_DIR}/"
else
    echo "WARNING: rsync not found; falling back to cp. This may recopy unchanged files." >&2
    (
        cd "$SOURCE_DIR"
        find . -type d -print0 | while IFS= read -r -d '' dir; do
            mkdir -p "${TARGET_DIR}/${dir#./}"
        done
        find . -type f -print0 | while IFS= read -r -d '' file; do
            target="${TARGET_DIR}/${file#./}"
            case "$file" in
                *.mp3|*.MP3|*.jpg|*.JPG|*.jpeg|*.JPEG)
                    [ -e "$target" ] && continue
                    ;;
            esac
            cp -p "$file" "$target"
        done
    )
fi

chmod +x "${TARGET_DIR}/CardBrick.sh" 2>/dev/null || true

echo "Flushing writes to the SD card..."
sync

echo "Done. Eject the SHARE volume safely before removing the card."
