#!/usr/bin/env bash
# Build the installable PortMaster package from cardbrick-py.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PORTMASTER_DIR="$(cd "$HERE/.." && pwd)"
REPO_ROOT="$(cd "$PORTMASTER_DIR/../.." && pwd)"
APP_SRC="$REPO_ROOT/cardbrick-py"
SOURCE_PORT="$PORTMASTER_DIR/port/cardbrick"
DIST="$PORTMASTER_DIR/dist"
STAGE="$DIST/cardbrick"
PACKAGE_ZIP="$DIST/cardbrick.zip"
DECKS_DIR="$PORTMASTER_DIR/decks"

write_manifest() {
    local root="$1" manifest="$2" launcher="$3" port_dir="$4"
    (
        cd "$root"
        find "./$launcher" "./$port_dir" -type f ! -name PACKAGE_MANIFEST.sha256 \
            -print0 | sort -z | \
            while IFS= read -r -d '' file; do
                if command -v sha256sum >/dev/null 2>&1; then
                    sha256sum "$file"
                else
                    shasum -a 256 "$file"
                fi
            done
    ) > "$manifest"
}

DECK_FILES=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --deck)
            [ "$#" -ge 2 ] || { echo "ERROR: --deck requires a path" >&2; exit 2; }
            DECK_FILES+=("$2")
            shift
            ;;
        *)
            echo "ERROR: unknown option: $1" >&2
            exit 2
            ;;
    esac
    shift
done

if [ -n "${CARDBRICK_DECKS:-}" ]; then
    deck_list="$CARDBRICK_DECKS"
    while :; do
        case "$deck_list" in
            *'|'*)
                deck="${deck_list%%|*}"
                deck_list="${deck_list#*|}"
                ;;
            *)
                deck="$deck_list"
                deck_list=""
                ;;
        esac
        [ -n "$deck" ] || {
            echo "ERROR: empty deck path in CARDBRICK_DECKS" >&2
            exit 2
        }
        DECK_FILES+=("$deck")
        [ -n "$deck_list" ] || break
    done
fi

if [ "${#DECK_FILES[@]}" -eq 0 ] && [ -d "$DECKS_DIR" ]; then
    while IFS= read -r -d '' file; do
        DECK_FILES+=("$file")
    done < <(find "$DECKS_DIR" -maxdepth 1 -type f \
        \( -iname '*.apkg' -o -iname '*.csv' \) -print0 | sort -z)
fi
if [ "${#DECK_FILES[@]}" -gt 0 ]; then
    for file in "${DECK_FILES[@]}"; do
        [ -f "$file" ] || { echo "ERROR: deck file not found: $file" >&2; exit 1; }
    done
fi

[ -f "$APP_SRC/main.py" ] || { echo "ERROR: CardBrick-Py is missing" >&2; exit 1; }
[ -x "$PORTMASTER_DIR/runtime-build/build/runtime.aarch64/bin/python3" ] || {
    echo "ERROR: build the ARM64 runtime first: make runtime" >&2
    exit 1
}

VERSION="$(sed -n 's/^__version__ *= *"\([^"]*\)".*/\1/p' \
    "$APP_SRC/cardbrick/__init__.py" | head -n 1)"
VERSION="${VERSION:-0.0.0}"

rm -rf "$STAGE" "$PACKAGE_ZIP"
rm -f "$DIST"/cardbrick-v*.zip
mkdir -p "$STAGE/cardbrick"

# Port metadata and launcher live at the package root; the installed port
# directory is the nested cardbrick/ directory listed in items.
cp "$SOURCE_PORT/CardBrick.sh" "$STAGE/"
cp "$SOURCE_PORT/cardbrick.port.json" "$STAGE/"
cp "$SOURCE_PORT/README.md" "$STAGE/"
cp "$SOURCE_PORT/gameinfo.xml" "$STAGE/"
chmod 755 "$STAGE/CardBrick.sh"

# Copy only application files that are part of the shipped program.
mkdir -p "$STAGE/cardbrick/cardbrick-py"
cp "$APP_SRC/main.py" "$APP_SRC/requirements.txt" "$STAGE/cardbrick/cardbrick-py/"
for directory in cardbrick assets scripts; do
    [ -d "$APP_SRC/$directory" ] || continue
    (cd "$APP_SRC" && tar cf - "$directory") | \
        (cd "$STAGE/cardbrick/cardbrick-py" && tar xf -)
done

# Add repository-level fonts, sounds, and artwork expected by CardBrick's
# asset lookup. The package gets a metadata screenshot from existing artwork
# until a hardware capture is available.
mkdir -p "$STAGE/cardbrick/cardbrick-py/assets/fonts" \
    "$STAGE/cardbrick/cardbrick-py/assets/sfx" \
    "$STAGE/cardbrick/licenses"
for font in NotoSansJP-Regular.otf NotoSansCJK-Regular.ttc PixelMplus10-Regular.ttf; do
    [ -f "$REPO_ROOT/assets/font/$font" ] && \
        cp "$REPO_ROOT/assets/font/$font" "$STAGE/cardbrick/cardbrick-py/assets/fonts/"
done
for sound in "$REPO_ROOT"/assets/sfx/*; do
    [ -f "$sound" ] && cp "$sound" "$STAGE/cardbrick/cardbrick-py/assets/sfx/"
done
[ -f "$REPO_ROOT/assets/icon-large.png" ] && \
    cp "$REPO_ROOT/assets/icon-large.png" "$STAGE/cardbrick/cardbrick-py/assets/icon-large.png"
[ -f "$REPO_ROOT/assets/icon-large.png" ] && \
    cp "$REPO_ROOT/assets/icon-large.png" "$STAGE/screenshot.png"
[ -f "$REPO_ROOT/assets/icon-large.png" ] && \
    cp "$REPO_ROOT/assets/icon-large.png" "$STAGE/cardbrick/screenshot.png"
cp "$SOURCE_PORT/licenses/LICENSE" "$STAGE/cardbrick/licenses/"
cp "$REPO_ROOT/LICENSE" "$STAGE/cardbrick/licenses/CardBrick-LICENSE.txt"

# Runtime is an ordinary directory, never a mounted image.
cp -a "$PORTMASTER_DIR/runtime-build/build/runtime.aarch64" \
    "$STAGE/cardbrick/runtime"

# Knulli's writable ports partition can be exFAT, which cannot represent
# Unix links. Materialize runtime library SONAME links as regular files and
# drop the venv's unused lib64 -> lib convenience link.
RUNTIME_STAGE="$STAGE/cardbrick/runtime"
[ ! -L "$RUNTIME_STAGE/lib64" ] || rm "$RUNTIME_STAGE/lib64"
while link="$(find "$RUNTIME_STAGE" -type l -print -quit)"; do
    [ -n "$link" ] || break
    [ ! -d "$link" ] || {
        echo "ERROR: unsupported directory symlink in runtime: $link" >&2
        exit 1
    }
    materialized="${link}.materialized"
    cp -pL "$link" "$materialized"
    rm "$link"
    mv "$materialized" "$link"
done

# macOS can attach provenance xattrs during Docker bind-mount copies. Do not
# let BSD tar turn those into AppleDouble/PAX records for the handheld.
if command -v xattr >/dev/null 2>&1; then
    xattr -cr "$STAGE" 2>/dev/null || true
fi

find "$STAGE" \( -name '__pycache__' -o -name '*.pyc' -o -name '.DS_Store' -o -name '._*' \) \
    -delete
find "$STAGE" -type d -name tests -prune -exec rm -rf {} +

# Optional seed deck(s), imported by the bundled ARM64 runtime. This keeps
# FSRS behavior identical to the handheld instead of relying on host Python.
if [ "${#DECK_FILES[@]}" -gt 0 ]; then
    command -v docker >/dev/null 2>&1 || {
        echo "ERROR: Docker is required to seed decks with the bundled ARM64 runtime" >&2
        exit 1
    }
    SEED_TMP="$(mktemp -d)"
    trap 'rm -rf "$SEED_TMP"' EXIT
    for file in "${DECK_FILES[@]}"; do
        docker run --rm --platform linux/arm64 \
            -v "$STAGE/cardbrick:/cardbrick" \
            -v "$SEED_TMP:/seed" \
            -v "$file:/deck/input:ro" \
            debian:bullseye bash -lc '
                apt-get update -qq && \
                apt-get install -y -qq libsdl2-2.0-0 libasound2 libpulse0 >/dev/null && \
                export PYTHONHOME=/cardbrick/runtime && \
                export PYTHONPATH=/cardbrick/runtime/lib/python3.12:/cardbrick/runtime/lib/python3.12/site-packages:/cardbrick/cardbrick-py && \
                export LD_LIBRARY_PATH=/cardbrick/runtime/lib && \
                export SDL_VIDEODRIVER=dummy && \
                export SDL_AUDIODRIVER=dummy && \
                /cardbrick/runtime/bin/python3 /cardbrick/cardbrick-py/main.py --data-dir /seed import /deck/input'
    done
    mkdir -p "$STAGE/cardbrick/seed-data"
    cp "$SEED_TMP/cardbrick.db" "$STAGE/cardbrick/seed-data/"
    [ ! -d "$SEED_TMP/media" ] || cp -a "$SEED_TMP/media" "$STAGE/cardbrick/seed-data/"
    {
        echo "built=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        for file in "${DECK_FILES[@]}"; do
            echo "source=$(basename "$file")"
        done
    } > "$STAGE/cardbrick/seed-data/SEED_INFO"
    rm -rf "$SEED_TMP"
    trap - EXIT
fi

# Importing seed decks executes the bundled interpreter and can recreate
# bytecode caches inside the staged runtime. Remove them before checksumming.
find "$STAGE" \( -name '__pycache__' -o -name '*.pyc' -o -name '.DS_Store' -o -name '._*' \) \
    -delete

echo "$VERSION" > "$STAGE/cardbrick/VERSION"
{
    echo "version=$VERSION"
    echo "built=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "git=$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
    echo "runtime=runtime.aarch64"
} > "$STAGE/cardbrick/BUILD_INFO"

write_manifest "$STAGE" "$STAGE/cardbrick/PACKAGE_MANIFEST.sha256" \
    CardBrick.sh cardbrick

if command -v zip >/dev/null 2>&1; then
    (cd "$STAGE" && rm -f "$PACKAGE_ZIP" && \
        zip -qr "$PACKAGE_ZIP" \
            CardBrick.sh cardbrick cardbrick.port.json README.md \
            gameinfo.xml screenshot.png)
fi

echo "Stage: $STAGE"
[ -f "$PACKAGE_ZIP" ] && echo "Zip: $PACKAGE_ZIP"
