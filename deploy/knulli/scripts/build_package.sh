#!/usr/bin/env bash
# Assemble the deployable CardBrick package for Knulli / RG35XX SP.
#
#   deploy/knulli/dist/
#   ├── CardBrick.sh                 # goes to /userdata/roms/ports/
#   ├── CardBrick/                   # goes to /userdata/roms/ports/
#   │   ├── cardbrick-py/            # the app, verbatim (no tests/caches)
#   │   ├── runtime/pygame-ce_*.squashfs
#   │   ├── seed-data/                   # OPTIONAL: pre-imported deck(s),
#   │   │   ├── cardbrick.db             # copied into place by CardBrick.sh
#   │   │   ├── media/                   # on first boot (only if no db
#   │   │   └── SEED_INFO                # exists yet in the data dir)
#   │   ├── VERSION  BUILD_INFO  PACKAGE_MANIFEST.sha256
#   └── CardBrick-knulli-v<ver>.zip  # same content, zipped for SD card
#
# Usage:
#   scripts/build_package.sh                 # runtime from runtime-build/build/
#   RUNTIME_SQUASHFS=/path/x.squashfs scripts/build_package.sh
#   scripts/build_package.sh --no-runtime    # app-only (fast re-deploys of
#                                            # code onto a device that
#                                            # already has the runtime)
#   scripts/build_package.sh --deck a.apkg --deck b.apkg
#                                            # bake deck(s) in: CardBrick.sh
#                                            # installs them into the data
#                                            # dir on the device's first
#                                            # boot, no on-device import
#                                            # step needed. Repeatable. With
#                                            # no --deck flag, every *.apkg
#                                            # / *.csv in deploy/knulli/decks/
#                                            # is used automatically (empty
#                                            # or absent -> no seed data,
#                                            # same as before this option
#                                            # existed).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KNULLI_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${KNULLI_DIR}/../.." && pwd)"
APP_SRC="${REPO_ROOT}/cardbrick-py"
DIST="${KNULLI_DIR}/dist"
STAGE="${DIST}/CardBrick"
DECKS_DIR="${KNULLI_DIR}/decks"

WITH_RUNTIME=1
DECK_FILES=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --no-runtime) WITH_RUNTIME=0 ;;
        --deck)
            [ "$#" -ge 2 ] || { echo "ERROR: --deck requires a path" >&2; exit 2; }
            DECK_FILES+=("$2")
            shift ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

# No explicit --deck? Fall back to whatever is sitting in decks/.
if [ "${#DECK_FILES[@]}" -eq 0 ] && [ -d "$DECKS_DIR" ]; then
    while IFS= read -r -d '' f; do
        DECK_FILES+=("$f")
    done < <(find "$DECKS_DIR" -maxdepth 1 -type f \
                  \( -iname '*.apkg' -o -iname '*.csv' \) -print0 | sort -z)
fi
for f in "${DECK_FILES[@]+"${DECK_FILES[@]}"}"; do
    [ -f "$f" ] || { echo "ERROR: deck file not found: $f" >&2; exit 1; }
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

# Copy an ALLOWLIST of what actually ships (matches the tree documented
# in package_layout.md), rather than the whole cardbrick-py/ directory
# minus excludes. A denylist misses local dev cruft that doesn't have a
# fixed name or location — most commonly a virtualenv created inside
# cardbrick-py/ (.venv, venv, env), whose installed packages routinely
# bundle their own nested tests/ and __pycache__ dirs. Copying only the
# known app paths makes that class of leak structurally impossible.
mkdir -p "${STAGE}/cardbrick-py"
cp "${APP_SRC}/main.py" "${STAGE}/cardbrick-py/"
[ -f "${APP_SRC}/requirements.txt" ] && cp "${APP_SRC}/requirements.txt" "${STAGE}/cardbrick-py/"
[ -f "${APP_SRC}/README.md" ] && cp "${APP_SRC}/README.md" "${STAGE}/cardbrick-py/"
for d in cardbrick assets scripts; do
    [ -d "${APP_SRC}/${d}" ] || continue
    ( cd "${APP_SRC}" && tar cf - "$d" ) | ( cd "${STAGE}/cardbrick-py" && tar xf - )
done
# Belt-and-suspenders: strip any cache dirs that did come from a tracked
# path above (e.g. a stray __pycache__ committed by accident).
find "${STAGE}/cardbrick-py" \
    \( -type d -name '__pycache__' -o -type d -name '.pytest_cache' \) \
    -exec rm -rf {} +
find "${STAGE}/cardbrick-py" -type f \( -name '*.pyc' -o -name '.DS_Store' \) -delete

install -m 755 "${KNULLI_DIR}/CardBrick.sh" "${DIST}/CardBrick.sh"

if [ -n "$RUNTIME_FILE" ]; then
    mkdir -p "${STAGE}/runtime"
    cp "$RUNTIME_FILE" "${STAGE}/runtime/"
    RUNTIME_MANIFEST="$(dirname "$RUNTIME_FILE")/runtime-manifest.txt"
    [ -f "$RUNTIME_MANIFEST" ] && cp "$RUNTIME_MANIFEST" "${STAGE}/runtime/"
fi

# --------------------------------------------------------- seed deck(s)
# Import the requested .apkg/.csv file(s) into a scratch data dir on
# THIS machine (main.py's "import" command needs only fsrs — no pygame,
# no ARM64 runtime — so this runs directly on the host) and ship the
# resulting database as seed-data/. CardBrick.sh copies it into place
# on the device's first boot, so the deck(s) are already installed with
# no on-device import step. Imports are additive (see PACKAGING.md), so
# multiple files just accumulate into one database.
if [ "${#DECK_FILES[@]}" -gt 0 ]; then
    DECK_PY="python3"
    if ! "$DECK_PY" -c "import fsrs" >/dev/null 2>&1; then
        VENV_DIR="${HERE}/.deckbuild-venv"
        if [ ! -x "${VENV_DIR}/bin/python3" ]; then
            echo "Setting up a local venv for deck import (needs fsrs)..."
            python3 -m venv "$VENV_DIR"
            "${VENV_DIR}/bin/pip" install -q --upgrade pip
            "${VENV_DIR}/bin/pip" install -q fsrs typing-extensions
        fi
        DECK_PY="${VENV_DIR}/bin/python3"
    fi

    SEED_TMP="$(mktemp -d)"
    trap '[ -n "${SEED_TMP:-}" ] && rm -rf "$SEED_TMP"' EXIT
    echo "Seeding ${#DECK_FILES[@]} deck file(s):"
    for f in "${DECK_FILES[@]}"; do
        echo "  - $(basename "$f")"
        "$DECK_PY" -B "${STAGE}/cardbrick-py/main.py" \
            --data-dir "$SEED_TMP" import "$f"
    done

    mkdir -p "${STAGE}/seed-data"
    cp "${SEED_TMP}/cardbrick.db" "${STAGE}/seed-data/"
    if [ -d "${SEED_TMP}/media" ] && [ -n "$(ls -A "${SEED_TMP}/media" 2>/dev/null)" ]; then
        cp -R "${SEED_TMP}/media" "${STAGE}/seed-data/"
    fi
    {
        echo "built=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        for f in "${DECK_FILES[@]}"; do echo "source=$(basename "$f")"; done
        echo "----"
        "$DECK_PY" -B "${STAGE}/cardbrick-py/main.py" --data-dir "$SEED_TMP" decks
    } > "${STAGE}/seed-data/SEED_INFO"

    rm -rf "$SEED_TMP"
    trap - EXIT

    # -B should already have prevented this, but running main.py against
    # the staged tree is exactly the situation that class of leak comes
    # from — re-run the cleanup so a bytecode cache never survives into
    # the checksum/zip regardless.
    find "${STAGE}/cardbrick-py" -type d -name '__pycache__' -exec rm -rf {} +

    echo "Seed data staged: ${STAGE}/seed-data"
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
