#!/usr/bin/env bash
# Validate the staged CardBrick package (deploy/minui/dist/) BEFORE it
# touches a device. Prints one [PASS]/[WARN]/[FAIL] line per check, in
# the same spirit as the app's own --smoke-test; exits non-zero if any
# hard check failed.
#
# Usage: scripts/validate_package.sh [path-to-dist]   (default: ../dist)

set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST="${1:-$(cd "${HERE}/.." && pwd)/dist}"
STAGE="${DIST}/CardBrick"

FAILURES=0
pass() { echo "[PASS] $*"; }
warn() { echo "[WARN] $*"; }
fail() { echo "[FAIL] $*"; FAILURES=$((FAILURES + 1)); }

[ -d "$STAGE" ] || { fail "stage dir missing: ${STAGE} — run build_package.sh first"; exit 1; }

# ------------------------------------------------------ launch script
LAUNCHER="${DIST}/CardBrick.sh"
if [ -f "$LAUNCHER" ]; then
    if sh -n "$LAUNCHER" 2>/dev/null; then
        pass "CardBrick.sh: sh syntax OK"
    else
        fail "CardBrick.sh: syntax error (sh -n)"
    fi
    if [ -x "$LAUNCHER" ]; then
        pass "CardBrick.sh: executable bit set"
    else
        fail "CardBrick.sh: not executable (chmod +x needed)"
    fi
else
    fail "CardBrick.sh missing from ${DIST}"
fi

# -------------------------------------------------------- app content
for f in \
    "cardbrick-py/main.py" \
    "cardbrick-py/cardbrick/__init__.py" \
    "cardbrick-py/cardbrick/app.py" \
    "cardbrick-py/cardbrick/paths.py" \
    "cardbrick-py/cardbrick/sync.py" \
    "cardbrick-py/cardbrick/smoke.py" \
    "cardbrick-py/assets/fonts/DejaVuSans.ttf" \
    "cardbrick-py/assets/fonts/NotoSansJP-Regular.otf" \
    "cardbrick-py/assets/fonts/NotoSans-OFL.txt" \
    "VERSION" "BUILD_INFO" "PACKAGE_MANIFEST.sha256"
do
    if [ -e "${STAGE}/${f}" ]; then
        pass "present: ${f}"
    else
        fail "missing: ${f}"
    fi
done

STRAY="$(find "$STAGE" \( -name '__pycache__' -o -name '*.pyc' -o -path '*/tests/*' \) \
        -print 2>/dev/null | head -n 5)"
if [ -n "$STRAY" ]; then
    fail "bundle contains __pycache__/.pyc/tests files (should be excluded):"
    echo "$STRAY" | sed 's/^/       /'
else
    pass "no __pycache__/.pyc/tests in bundle"
fi

# All Python sources must at least compile with the host interpreter.
if command -v python3 >/dev/null 2>&1; then
    PYCACHE="/tmp/cardbrick-package-pycache-$$"
    rm -rf "$PYCACHE"
    if ( cd "${STAGE}/cardbrick-py" && \
         find . -name '*.py' -print0 | \
         xargs -0 env PYTHONPYCACHEPREFIX="$PYCACHE" \
             python3 -m py_compile 2>/tmp/pyc.err ); then
        pass "python sources compile (host python3)"
    else
        fail "py_compile errors: $(head -n 3 /tmp/pyc.err 2>/dev/null | tr '\n' ' ')"
    fi
    # py_compile with default args writes __pycache__ next to sources — undo.
    find "${STAGE}/cardbrick-py" -name '__pycache__' -type d -exec rm -rf {} + 2>/dev/null
    rm -rf "$PYCACHE"
else
    warn "python3 not on host — skipped source compile check"
fi

# ------------------------------------------------------------ runtime
RUNTIME_FILE="$(ls -1 "${STAGE}/runtime/"pygame-ce_*.squashfs 2>/dev/null | head -n 1)"
if [ -n "$RUNTIME_FILE" ]; then
    pass "runtime present: $(basename "$RUNTIME_FILE") ($(du -h "$RUNTIME_FILE" | cut -f1))"

    # squashfs superblock magic: 'hsqs'
    MAGIC="$(head -c 4 "$RUNTIME_FILE" 2>/dev/null)"
    if [ "$MAGIC" = "hsqs" ]; then
        pass "runtime: valid squashfs magic"
    else
        fail "runtime: not a squashfs image (bad magic)"
    fi

    if command -v unsquashfs >/dev/null 2>&1; then
        if unsquashfs -s "$RUNTIME_FILE" >/dev/null 2>&1; then
            pass "runtime: unsquashfs superblock check OK"
        else
            fail "runtime: unsquashfs cannot read the image"
        fi
        LISTING="$(unsquashfs -l "$RUNTIME_FILE" 2>/dev/null)"
        for want in "/bin/python3" "/lib/libpython" "site-packages/pygame"; do
            if echo "$LISTING" | grep -q "$want"; then
                pass "runtime contains ${want}"
            else
                fail "runtime is missing ${want}"
            fi
        done
        if echo "$LISTING" | grep -q "site-packages/fsrs"; then
            pass "runtime contains fsrs"
        else
            fail "runtime is missing fsrs (was runtime-build/requirements.txt applied?)"
        fi
    else
        warn "unsquashfs not on host — install squashfs-tools for deep runtime checks"
    fi
else
    warn "no runtime squashfs in bundle (--no-runtime build?) — device must already have one"
fi

# ---------------------------------------------------------- seed deck(s)
SEED_DB="${STAGE}/seed-data/cardbrick.db"
if [ -f "$SEED_DB" ]; then
    if command -v python3 >/dev/null 2>&1; then
        SEED_CHECK="$(python3 - "$SEED_DB" <<'PYEOF'
import sqlite3
import sys

db = sys.argv[1]
try:
    con = sqlite3.connect(db)
    ok = con.execute("PRAGMA integrity_check").fetchone()[0]
    if ok != "ok":
        print(f"CORRUPT:{ok}")
        sys.exit(0)
    count = con.execute("SELECT COUNT(*) FROM cards").fetchone()[0]
    print(f"OK:{count}")
except Exception as exc:  # noqa: BLE001 - reporting only
    print(f"ERROR:{exc}")
PYEOF
)"
        case "$SEED_CHECK" in
            OK:0) fail "seed-data DB has 0 cards — deck import likely failed silently" ;;
            OK:*) pass "seed-data DB OK: ${SEED_CHECK#OK:} card(s) pre-installed" ;;
            *) fail "seed-data DB check failed: ${SEED_CHECK}" ;;
        esac
    else
        warn "python3 not on host — skipped seed-data DB integrity check"
    fi
    [ -f "${STAGE}/seed-data/SEED_INFO" ] \
        && pass "seed-data/SEED_INFO present" \
        || fail "seed-data/SEED_INFO missing"
elif [ -d "${STAGE}/seed-data" ]; then
    fail "seed-data/ exists but cardbrick.db is missing"
else
    warn "no seed-data — package ships with no decks pre-installed" \
         "(rebuild with --deck FILE.apkg, or import on-device via Parent Mode)"
fi

# ----------------------------------------------------------- checksums
if command -v sha256sum >/dev/null 2>&1 && [ -f "${STAGE}/PACKAGE_MANIFEST.sha256" ]; then
    if ( cd "$STAGE" && sha256sum --quiet -c PACKAGE_MANIFEST.sha256 2>/dev/null ); then
        pass "PACKAGE_MANIFEST.sha256 verifies"
    else
        fail "checksum mismatch against PACKAGE_MANIFEST.sha256"
    fi
fi

# ------------------------------------------------------------------ zip
ZIP="$(ls -1t "${DIST}/"CardBrick-minui-*.zip 2>/dev/null | head -n 1)"
if [ -n "$ZIP" ]; then
    if command -v unzip >/dev/null 2>&1; then
        if unzip -tqq "$ZIP" >/dev/null 2>&1; then
            pass "zip integrity OK: $(basename "$ZIP")"
        else
            fail "zip is corrupt: $(basename "$ZIP")"
        fi
    else
        warn "unzip not on host — skipped zip integrity check"
    fi
fi

echo
if [ "$FAILURES" -eq 0 ]; then
    echo "RESULT: PASS — package is ready to deploy."
else
    echo "RESULT: FAIL — ${FAILURES} problem(s) above must be fixed first."
    exit 1
fi
