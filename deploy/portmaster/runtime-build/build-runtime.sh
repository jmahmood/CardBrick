#!/usr/bin/env bash
# Build the extracted ARM64 runtime inside Debian Bullseye.

set -euo pipefail

: "${PYTHON_VERSION:?PYTHON_VERSION is required}"
: "${PYTHON_SHA256:?PYTHON_SHA256 is required}"
: "${PYGAME_VERSION:?PYGAME_VERSION is required}"
: "${PYGAME_COMMIT:?PYGAME_COMMIT is required}"

PY_VER="${PYTHON_VERSION%.*}"
BUILD_DIR="/tmp/cardbrick-runtime-build"
PY_PREFIX="/opt/python-${PYTHON_VERSION}"
RUNTIME_DIR="/local/build/runtime.aarch64"

rm -rf "$BUILD_DIR" "$PY_PREFIX" "$RUNTIME_DIR"
mkdir -p "$BUILD_DIR" "$RUNTIME_DIR/lib"

apt-get update
apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl git rsync pkg-config patchelf ninja-build \
    file xz-utils zlib1g-dev libbz2-dev libreadline-dev libsqlite3-dev \
    libffi-dev libssl-dev liblzma-dev libncursesw5-dev libgdbm-dev uuid-dev \
    libfreetype6-dev libportmidi-dev libjpeg-dev libpng-dev \
    libsdl2-dev libsdl2-image-dev libsdl2-mixer-dev libsdl2-ttf-dev

cd "$BUILD_DIR"
curl -fsSLO "https://www.python.org/ftp/python/${PYTHON_VERSION}/Python-${PYTHON_VERSION}.tgz"
printf '%s  %s\n' "$PYTHON_SHA256" "Python-${PYTHON_VERSION}.tgz" | sha256sum -c -
tar xf "Python-${PYTHON_VERSION}.tgz"
cd "Python-${PYTHON_VERSION}"

./configure \
    --prefix="$PY_PREFIX" \
    --enable-shared \
    --with-ensurepip=install
make -j"$(nproc)"
make install

export LD_LIBRARY_PATH="$PY_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
"$PY_PREFIX/bin/python3" --version

"$PY_PREFIX/bin/python3" -m venv --copies "$RUNTIME_DIR"
export LD_LIBRARY_PATH="$RUNTIME_DIR/lib:$PY_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
source "$RUNTIME_DIR/bin/activate"

python -m pip install --no-cache-dir \
    pip==24.3.1 setuptools==75.8.0 wheel==0.45.1
cd "$BUILD_DIR"
git init pygame-ce
cd pygame-ce
git remote add origin https://github.com/pygame-community/pygame-ce.git
git fetch --depth 1 origin "$PYGAME_COMMIT"
git checkout --detach "$PYGAME_COMMIT"
[ "$(git rev-parse HEAD)" = "$PYGAME_COMMIT" ] || {
    echo "ERROR: pygame-ce source revision does not match $PYGAME_COMMIT" >&2
    exit 1
}

# pygame-ce uses Meson. Build its wheel in a disposable, version-pinned
# environment so compiler tooling is not shipped in the handheld runtime.
deactivate
"$PY_PREFIX/bin/python3" -m venv "$BUILD_DIR/pygame-build-env"
source "$BUILD_DIR/pygame-build-env/bin/activate"
python -m pip install --no-cache-dir \
    pip==24.3.1 setuptools==75.8.0 wheel==0.45.1 \
    Cython==3.0.11 meson==1.7.0 meson-python==0.17.1 \
    packaging==26.2 pyproject-metadata==0.9.1
python -m pip wheel --no-build-isolation --no-deps \
    --wheel-dir "$BUILD_DIR/wheels" .
deactivate
source "$RUNTIME_DIR/bin/activate"
python -m pip install --no-cache-dir --no-deps "$BUILD_DIR"/wheels/pygame_ce-*.whl
python -m pip install --no-cache-dir --no-deps -r /local/requirements.txt

python - <<'PY'
import fsrs
import pygame
import sqlite3
import sys

print("python", sys.version)
print("pygame", pygame.version.ver)
print("fsrs", getattr(fsrs, "__version__", "installed"))
print("sqlite", sqlite3.sqlite_version)
PY

deactivate

cp "$PY_PREFIX/lib/libpython${PY_VER}.so.1.0" "$RUNTIME_DIR/lib/"
rsync -a "$PY_PREFIX/lib/python${PY_VER}/" "$RUNTIME_DIR/lib/python${PY_VER}/"

mkdir -p "$RUNTIME_DIR/licenses/debian"

# Keep the firmware's SDL2 core so PortMaster-supported video/input backends
# remain available. Everything else required by an ELF in the runtime is
# copied recursively from Bullseye and must resolve inside runtime/lib.
is_firmware_dependency() {
    case "$1" in
        libc.so.6|libm.so.6|libdl.so.2|libpthread.so.0|librt.so.1|ld-linux-aarch64.so.1|libSDL2-2.0.so.0|libgcc_s.so.1|libstdc++.so.6)
            return 0
            ;;
    esac
    return 1
}

needed_libraries() {
    readelf -d "$1" 2>/dev/null | sed -n 's/.*NEEDED.*\[\([^]]*\)\].*/\1/p'
}

runtime_elf_candidates() {
    find "$RUNTIME_DIR" -type f \
        \( -name '*.so' -o -name '*.so.*' -o -path "$RUNTIME_DIR/bin/python*" \) \
        -print0
}

find_system_library() {
    local name="$1" candidate
    for candidate in \
        "/lib/aarch64-linux-gnu/$name" \
        "/usr/lib/aarch64-linux-gnu/$name" \
        "/usr/local/lib/$name"; do
        [ -e "$candidate" ] && { printf '%s\n' "$candidate"; return 0; }
    done
    candidate="$(ldconfig -p 2>/dev/null | awk -v library="$name" '$1 == library { print $NF; exit }')"
    [ -n "$candidate" ] && [ -e "$candidate" ] && {
        printf '%s\n' "$candidate"
        return 0
    }
    for candidate in \
        "$(find /lib/aarch64-linux-gnu /usr/lib/aarch64-linux-gnu -type f -name "$name" -print -quit 2>/dev/null)" \
        "$(find /lib/aarch64-linux-gnu /usr/lib/aarch64-linux-gnu -type l -name "$name" -print -quit 2>/dev/null)"; do
        [ -n "$candidate" ] && [ -e "$candidate" ] && {
            printf '%s\n' "$candidate"
            return 0
        }
    done
    return 1
}

copy_debian_license() {
    local source package copyright
    source="$(readlink -f "$1")"
    package="$(dpkg-query -S "$source" 2>/dev/null | head -n 1 | cut -d: -f1 || true)"
    package="${package%%,*}"
    [ -n "$package" ] || return 0
    package="${package%%:*}"
    copyright="/usr/share/doc/$package/copyright"
    [ -f "$copyright" ] && cp -n "$copyright" "$RUNTIME_DIR/licenses/debian/$package-copyright" || true
}

copy_runtime_library() {
    local needed="$1" source resolved soname
    source="$(find_system_library "$needed")" || {
        echo "ERROR: unable to resolve non-firmware dependency $needed" >&2
        return 1
    }
    resolved="$(readlink -f "$source")"
    cp -n -L "$resolved" "$RUNTIME_DIR/lib/"
    if [ "$needed" != "$(basename "$resolved")" ]; then
        ln -sf "$(basename "$resolved")" "$RUNTIME_DIR/lib/$needed"
    fi
    soname="$(readelf -d "$resolved" 2>/dev/null | sed -n 's/.*SONAME.*\[\([^]]*\)\].*/\1/p' | head -n 1)"
    if [ -n "$soname" ] && [ "$soname" != "$(basename "$resolved")" ]; then
        ln -sf "$(basename "$resolved")" "$RUNTIME_DIR/lib/$soname"
    fi
    copy_debian_license "$source"
}

echo "== Runtime dependency closure =="
while :; do
    copied=0
    while IFS= read -r -d '' elf; do
        while IFS= read -r needed; do
            is_firmware_dependency "$needed" && continue
            [ -e "$RUNTIME_DIR/lib/$needed" ] && continue
            copy_runtime_library "$needed"
            copied=1
        done < <(needed_libraries "$elf")
    done < <(runtime_elf_candidates)
    [ "$copied" -eq 0 ] && break
done

unresolved=0
while IFS= read -r -d '' elf; do
    while IFS= read -r needed; do
        is_firmware_dependency "$needed" && continue
        if [ ! -e "$RUNTIME_DIR/lib/$needed" ]; then
            echo "ERROR: unresolved runtime dependency $needed (needed by $elf)" >&2
            unresolved=1
        fi
    done < <(needed_libraries "$elf")
done < <(runtime_elf_candidates)
[ "$unresolved" -eq 0 ] || exit 1

export LD_LIBRARY_PATH="$RUNTIME_DIR/lib:$PY_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

find "$RUNTIME_DIR" -type d -name '__pycache__' -prune -exec rm -rf {} +

mkdir -p "$RUNTIME_DIR/licenses"
cp "$BUILD_DIR/Python-${PYTHON_VERSION}/LICENSE" \
    "$RUNTIME_DIR/licenses/Python-LICENSE.txt"
if [ -f "$BUILD_DIR/pygame-ce/LICENSE.txt" ]; then
    cp "$BUILD_DIR/pygame-ce/LICENSE.txt" \
        "$RUNTIME_DIR/licenses/pygame-ce-LICENSE.txt"
elif [ -f "$BUILD_DIR/pygame-ce/LICENSE" ]; then
    cp "$BUILD_DIR/pygame-ce/LICENSE" \
        "$RUNTIME_DIR/licenses/pygame-ce-LICENSE.txt"
fi
[ ! -f /usr/share/doc/libsdl2-dev/copyright ] || \
    cp /usr/share/doc/libsdl2-dev/copyright "$RUNTIME_DIR/licenses/SDL2-copyright"
for package in fsrs typing_extensions packaging; do
    license="$(find "$RUNTIME_DIR/lib/python${PY_VER}/site-packages" \
        -path "*/${package}-*.dist-info/licenses/*" -type f -print -quit 2>/dev/null || true)"
    [ -n "$license" ] && cp "$license" "$RUNTIME_DIR/licenses/${package}-LICENSE.txt"
done
cat > "$RUNTIME_DIR/licenses/DEPENDENCIES.txt" <<EOF
CardBrick runtime dependency notices

CardBrick: see the repository LICENSE file.
CPython: Python Software Foundation License; see Python-LICENSE.txt.
pygame-ce: LGPL-2.1-or-later; source project metadata is included above.
fsrs: MIT; see fsrs-LICENSE.txt.
typing-extensions: Python Software Foundation License; see typing_extensions-LICENSE.txt.
packaging: Apache-2.0 or BSD-2-Clause; see packaging-LICENSE.txt.
SDL2 family: LGPL-2.1-or-later; see SDL2-copyright when available.
SQLite: public domain.
libgcc/libstdc++: provided by the PortMaster firmware to match its GPU/driver ABI.
Other copied Debian shared libraries: see licenses/debian/.
EOF

cat > "$RUNTIME_DIR/runtime-manifest.txt" <<EOF
python=${PYTHON_VERSION}
pygame-ce=${PYGAME_VERSION}
base=debian-bullseye-arm64
glibc_floor=2.31
sdl2=firmware-provided
pygame-ce_commit=${PYGAME_COMMIT}
build_time=$(date -u +%Y-%m-%dT%H:%M:%SZ)
---- pip freeze ----
EOF
"$RUNTIME_DIR/bin/python3" -m pip freeze | sed '/^pygame-ce @ /d' >> "$RUNTIME_DIR/runtime-manifest.txt"

echo "== Runtime smoke test =="
export PYTHONHOME="$RUNTIME_DIR"
export PYTHONPATH="$RUNTIME_DIR/lib/python${PY_VER}:$RUNTIME_DIR/lib/python${PY_VER}/site-packages"
export SDL_VIDEODRIVER=dummy
export SDL_AUDIODRIVER=dummy
"$RUNTIME_DIR/bin/python3" - <<'PY'
import fsrs
import pygame
import sqlite3

pygame.init()
pygame.display.set_mode((64, 48))
pygame.font.init()
pygame.font.Font(None, 12).render("CardBrick", True, (255, 255, 255))
print("runtime-smoke-ok", pygame.version.ver, sqlite3.sqlite_version)
pygame.quit()
PY

echo "== glibc symbol check =="
bad=0
max_seen=0
while IFS= read -r -d '' file; do
    while IFS= read -r symbol; do
        minor="${symbol#GLIBC_2.}"
        [ "$minor" = "$symbol" ] && continue
        if [ "$minor" -gt "$max_seen" ]; then
            max_seen="$minor"
        fi
        if [ "$minor" -gt 31 ]; then
            echo "ERROR: $file requires $symbol (maximum allowed GLIBC_2.31)"
            bad=1
        fi
    done < <(strings "$file" 2>/dev/null | grep -Eo 'GLIBC_2\.[0-9]+' | sort -Vu || true)
done < <(find "$RUNTIME_DIR" -type f \( -name '*.so' -o -name '*.so.*' -o -path '*/bin/python3*' \) -print0)

if [ "$bad" -ne 0 ]; then
    exit 1
fi
echo "maximum observed GLIBC_2.${max_seen}"
echo "runtime output: $RUNTIME_DIR"
