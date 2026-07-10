#!/usr/bin/env bash
# Validate the staged experimental PortMaster package.

set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PORTMASTER_DIR="$(cd "$HERE/.." && pwd)"
DIST="${1:-$PORTMASTER_DIR/dist}"
STAGE="$DIST/cardbrick"
FAILURES=0

pass() { echo "[PASS] $*"; }
warn() { echo "[WARN] $*"; }
fail() { echo "[FAIL] $*"; FAILURES=$((FAILURES + 1)); }

[ -d "$STAGE" ] || { fail "missing $STAGE; run make package first"; exit 1; }

for file in CardBrick.sh cardbrick.port.json README.md gameinfo.xml screenshot.png; do
    [ -f "$STAGE/$file" ] && pass "present: $file" || fail "missing: $file"
done

if bash -n "$STAGE/CardBrick.sh"; then
    pass "launcher syntax"
else
    fail "launcher syntax"
fi
[ -x "$STAGE/CardBrick.sh" ] && pass "launcher executable" || fail "launcher not executable"

if python3 - "$STAGE/cardbrick.port.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as fh:
    data = json.load(fh)
attr = data["attr"]
assert data["version"] == 2
assert data["name"] == "cardbrick.zip"
assert attr["arch"] == ["aarch64"]
assert attr["rtr"] is True
assert attr["runtime"] is None
assert "CardBrick.sh" in data["items"]
assert "cardbrick" in data["items"]
PY
then
    pass "PortMaster metadata"
else
    fail "PortMaster metadata"
fi

for file in \
    cardbrick/cardbrick-py/main.py \
    cardbrick/cardbrick-py/cardbrick/__init__.py \
    cardbrick/cardbrick-py/cardbrick/app.py \
    cardbrick/cardbrick-py/cardbrick/paths.py \
    cardbrick/runtime/bin/python3 \
    cardbrick/runtime/runtime-manifest.txt \
    cardbrick/BUILD_INFO \
    cardbrick/PACKAGE_MANIFEST.sha256 \
    cardbrick/licenses/LICENSE \
    cardbrick/licenses/CardBrick-LICENSE.txt; do
    [ -e "$STAGE/$file" ] && pass "present: $file" || fail "missing: $file"
done

STRAY="$(find "$STAGE" \( -name '__pycache__' -o -name '*.pyc' -o \
    -path '*/tests/*' -o -name '*.squashfs' \) -print 2>/dev/null | head -n 5)"
if [ -n "$STRAY" ]; then
    fail "package contains forbidden cache, test, or SquashFS files"
    echo "$STRAY" | sed 's/^/       /'
else
    pass "no caches, tests, or SquashFS files"
fi

LINKS="$(find "$STAGE" -type l -print 2>/dev/null | head -n 5)"
if [ -n "$LINKS" ]; then
    fail "package contains symlinks, which cannot be installed on exFAT ports partitions"
    echo "$LINKS" | sed 's/^/       /'
else
    pass "no symlinks (exFAT-compatible package)"
fi

if rg -n -i '/mnt/mmc|/opt/muos|export SDL_VIDEODRIVER=|export SDL_AUDIODRIVER=' \
    "$STAGE/CardBrick.sh" >/tmp/cardbrick-portmaster-validator.txt 2>/dev/null; then
    fail "launcher contains hard-coded CFW paths or SDL driver assignments"
    sed 's/^/       /' /tmp/cardbrick-portmaster-validator.txt
else
    pass "launcher uses PortMaster paths and leaves SDL drivers to the CFW"
fi

if command -v python3 >/dev/null 2>&1; then
    if python3 - "$STAGE/cardbrick/cardbrick-py" <<'PY' \
        2>/tmp/cardbrick-portmaster-pyc.err
from pathlib import Path
import sys

root = Path(sys.argv[1])
for path in root.rglob("*.py"):
    compile(path.read_text(encoding="utf-8"), str(path), "exec")
PY
    then
        pass "Python sources compile on host"
    else
        fail "Python source compilation failed"
        sed 's/^/       /' /tmp/cardbrick-portmaster-pyc.err
    fi
fi

RUNTIME="$STAGE/cardbrick/runtime"
if [ -x "$RUNTIME/bin/python3" ]; then
    if file "$RUNTIME/bin/python3" | grep -q 'ARM aarch64'; then
        pass "runtime interpreter is ARM64"
    else
        fail "runtime interpreter is not an ARM64 ELF"
    fi

    glibc_symbols="$({
        while IFS= read -r -d '' elf; do
            strings "$elf" 2>/dev/null || true
        done < <(find "$RUNTIME" -type f \
            \( -name '*.so' -o -name '*.so.*' -o -path "$RUNTIME/bin/python*" \) \
            -print0)
    } | grep -Eo 'GLIBC_2\.[0-9]+' | sort -Vu || true)"
    bad_glibc=0
    while IFS= read -r symbol; do
        [ -n "$symbol" ] || continue
        minor="${symbol#GLIBC_2.}"
        [ "$minor" -gt 31 ] && bad_glibc=1
    done <<< "$glibc_symbols"
    if [ "$bad_glibc" -eq 0 ]; then
        pass "runtime glibc symbols do not exceed 2.31"
    else
        fail "runtime requires glibc newer than 2.31"
    fi
else
    fail "bundled runtime interpreter missing"
fi

if [ -f "$STAGE/cardbrick/PACKAGE_MANIFEST.sha256" ]; then
    if (cd "$STAGE" && {
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum -c cardbrick/PACKAGE_MANIFEST.sha256
        else
            shasum -a 256 -c cardbrick/PACKAGE_MANIFEST.sha256
        fi
    } >/dev/null 2>&1); then
        pass "package checksum manifest"
    else
        fail "package checksum manifest"
    fi
fi

SEED_DB="$STAGE/cardbrick/seed-data/cardbrick.db"
if [ -f "$SEED_DB" ]; then
    if python3 - "$SEED_DB" <<'PY'
import sqlite3
import sys

with sqlite3.connect(sys.argv[1]) as con:
    assert con.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
    assert con.execute("SELECT COUNT(*) FROM cards").fetchone()[0] > 0
PY
    then
        pass "seed deck database integrity"
    else
        fail "seed deck database is corrupt or empty"
    fi
    [ -f "$STAGE/cardbrick/seed-data/SEED_INFO" ] && \
        pass "seed deck metadata" || fail "seed deck metadata missing"
elif [ -d "$STAGE/cardbrick/seed-data" ]; then
    fail "seed-data directory exists without cardbrick.db"
fi

ZIP="$DIST/cardbrick.zip"
if [ -n "$ZIP" ] && command -v unzip >/dev/null 2>&1; then
    unzip -tqq "$ZIP" >/dev/null 2>&1 && \
        pass "zip integrity: $(basename "$ZIP")" || \
        fail "zip integrity: $(basename "$ZIP")"
    if unzip -Z1 "$ZIP" | grep -qx 'CardBrick.sh' && \
       unzip -Z1 "$ZIP" | grep -qx 'cardbrick/runtime/bin/python3'; then
        pass "zip has PortMaster archive-root items"
    else
        fail "zip has incorrect archive-root layout"
    fi
fi

echo
if [ "$FAILURES" -eq 0 ]; then
    echo "RESULT: PASS"
else
    echo "RESULT: FAIL ($FAILURES problem(s))"
    exit 1
fi
