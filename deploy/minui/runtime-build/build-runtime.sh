#!/usr/bin/env bash
# Builds the pygame-ce squashfs runtime for CardBrick on MinUI (TrimUI
# Brick / tg5040), INSIDE the ARM64 builder container (see
# docker-compose.yml). Do not run on the host.
#
# Unlike deploy/knulli/runtime-build/build-runtime.sh, this compiles
# Python itself from source on Debian Bullseye rather than starting
# from a prebuilt CPython image: MinUI's libc caps out at GLIBC 2.33,
# and a Bullseye-built interpreter (plus everything linked against it)
# stays under that ceiling. The GLIBC symbol scan near the end of this
# script fails the build if anything sneaks past it.
set -euo pipefail

: "${PYTHON_VERSION:?PYTHON_VERSION is required}"
: "${PYGAME_VERSION:?PYGAME_VERSION is required}"

export PY_VER="${PYTHON_VERSION%.*}"

BUILD_DIR="/tmp/cardbrick-runtime-build"
PY_PREFIX="/opt/python-${PYTHON_VERSION}"
RUNTIME_DIR="/runtime"
OUT_NAME="pygame-ce_${PYGAME_VERSION}_python_${PYTHON_VERSION}_minui_bullseye_aarch64.squashfs"

cd "$HOME"

echo "== Installing build dependencies =="
apt-get update
apt-get install -y \
  build-essential \
  ca-certificates \
  curl \
  git \
  rsync \
  squashfs-tools \
  pkg-config \
  patchelf \
  file \
  xz-utils \
  zlib1g-dev \
  libbz2-dev \
  libreadline-dev \
  libsqlite3-dev \
  libffi-dev \
  libssl-dev \
  liblzma-dev \
  libncursesw5-dev \
  libgdbm-dev \
  uuid-dev \
  libfreetype6-dev \
  libportmidi-dev \
  libjpeg-dev \
  libpng-dev \
  libsdl2-dev \
  libsdl2-image-dev \
  libsdl2-mixer-dev \
  libsdl2-ttf-dev

if [[ -f /local/build/pre-install.sh ]]; then
  echo "== Running pre-install.sh =="
  bash /local/build/pre-install.sh
fi

echo "== Building Python ${PYTHON_VERSION} on Debian Bullseye =="
rm -rf "$BUILD_DIR" "$PY_PREFIX" "$RUNTIME_DIR"
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

curl -LO "https://www.python.org/ftp/python/${PYTHON_VERSION}/Python-${PYTHON_VERSION}.tgz"
tar xf "Python-${PYTHON_VERSION}.tgz"
cd "Python-${PYTHON_VERSION}"

./configure \
  --prefix="$PY_PREFIX" \
  --enable-shared \
  --with-ensurepip=install

make -j"$(nproc)"
make install

export LD_LIBRARY_PATH="${PY_PREFIX}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

echo "== Verifying compiled Python =="
"${PY_PREFIX}/bin/python3" --version
"${PY_PREFIX}/bin/python3" - <<'PY'
import sys
import sqlite3
print(sys.version)
print("sqlite", sqlite3.sqlite_version)
PY

echo "== Creating runtime venv =="
"${PY_PREFIX}/bin/python3" -m venv --copies "$RUNTIME_DIR"

# Make venv Python able to find the Python shared library while building.
export LD_LIBRARY_PATH="${RUNTIME_DIR}/lib:${PY_PREFIX}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

source "${RUNTIME_DIR}/bin/activate"

python -m pip install --upgrade pip setuptools wheel

echo "== Building pygame-ce ${PYGAME_VERSION} =="
cd "$BUILD_DIR"
git clone --branch "${PYGAME_VERSION}" --depth 1 https://github.com/pygame-community/pygame-ce.git
cd pygame-ce
python -m pip install .

if [[ -f /local/build/requirements.txt ]]; then
  echo "== Installing additional requirements =="
  python -m pip install -r /local/build/requirements.txt
fi

echo "== Verifying runtime imports =="
python - <<'PY'
import sys
import sqlite3
import pygame
print("python", sys.version)
print("sqlite", sqlite3.sqlite_version)
print("pygame", pygame.version.ver)
PY

deactivate

echo "== Copying Python shared library and stdlib into runtime =="
cp "${PY_PREFIX}/lib/libpython${PY_VER}.so.1.0" "${RUNTIME_DIR}/lib/"

echo "== Copying SQLite runtime library =="
cp -av /usr/lib/aarch64-linux-gnu/libsqlite3.so.* "${RUNTIME_DIR}/lib/"

# Copy full stdlib, including lib-dynload where _sqlite3 lives.
rsync -a "${PY_PREFIX}/lib/python${PY_VER}/" "${RUNTIME_DIR}/lib/python${PY_VER}/"

# Remove bytecode caches.
find "$RUNTIME_DIR" -name "__pycache__" -type d -prune -exec rm -rf {} +

if [[ -f /local/build/post-install.sh ]]; then
  echo "== Running post-install.sh =="
  bash /local/build/post-install.sh
fi

echo "== Final runtime validation =="
export PYTHONHOME="$RUNTIME_DIR"
export PYTHONPATH="${RUNTIME_DIR}/lib/python${PY_VER}:${RUNTIME_DIR}/lib/python${PY_VER}/site-packages"
export LD_LIBRARY_PATH="${RUNTIME_DIR}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

"${RUNTIME_DIR}/bin/python3" - <<'PY'
import sys
import sqlite3
import pygame
print("runtime-ok", sys.executable)
print("sqlite", sqlite3.sqlite_version)
print("pygame", pygame.version.ver)
PY

echo "== Checking for too-new GLIBC symbols =="
BAD_GLIBC=0

while IFS= read -r -d '' f; do
  max_symbol="$(strings "$f" 2>/dev/null | grep -Eo 'GLIBC_[0-9]+\.[0-9]+' | sort -Vu | sort -V | tail -n 1 || true)"
  if [[ -n "$max_symbol" ]]; then
    echo "$f -> $max_symbol"
    if echo "$max_symbol" | grep -Eq 'GLIBC_2\.3[4-9]|GLIBC_2\.[4-9][0-9]'; then
      echo "ERROR: too-new GLIBC requirement: $f -> $max_symbol"
      BAD_GLIBC=1
    fi
  fi
done < <(find "$RUNTIME_DIR" -type f \( -name '*.so' -o -name '*.so.*' -o -path '*/bin/python3*' \) -print0)

if [[ "$BAD_GLIBC" -ne 0 ]]; then
  echo "ERROR: Runtime requires GLIBC newer than MinUI's 2.33"
  exit 1
fi

echo "== Confirming sqlite3 extension exists =="
find "$RUNTIME_DIR" -name '_sqlite3*.so' -print
find "$RUNTIME_DIR" -name 'libsqlite3.so*' -print || true

echo "== Creating squashfs =="
rm -f "/${OUT_NAME}" "/local/build/${OUT_NAME}"

mkdir -p /local/build

# Use gzip for maximum kernel compatibility.
mksquashfs "$RUNTIME_DIR" "/${OUT_NAME}" -comp gzip -noappend

mksquashfs "$RUNTIME_DIR" "/${OUT_NAME}" -comp gzip -noappend

mv "/${OUT_NAME}" "/local/build/${OUT_NAME}"

echo "== Built /local/build/${OUT_NAME} =="
