#!/usr/bin/env bash
# Builds the pygame-ce squashfs runtime for CardBrick INSIDE the ARM64
# builder container (see docker-compose.yml). Do not run on the host.
#
# Derived from https://github.com/viniciusfs/pygame-ce-runtime
# (build-runtime.sh), adapted for CardBrick:
#   - installs the app's extra deps (/local/requirements.txt) into the
#     runtime so the device needs nothing but this one squashfs;
#   - writes build/runtime-manifest.txt (pip freeze + versions) so a
#     package can be traced back to exactly what is inside it.
#
# pygame-ce is deliberately built FROM SOURCE against Debian's SDL2
# development headers instead of installing the manylinux wheel: the
# wheel statically bundles its own SDL2, while a source build links
# libSDL2-2.0.so.0 dynamically — on the device that resolves to
# Knulli's firmware SDL2, which carries the mali/kmsdrm video drivers
# the handheld display actually needs.

set -euo pipefail

: "${PYTHON_VERSION:?PYTHON_VERSION is required}"
: "${PYGAME_VERSION:?PYGAME_VERSION is required}"

export PY_VER=${PYTHON_VERSION%.*}

cd "$HOME"

# install dependencies
apt update && apt install -y build-essential git rsync squashfs-tools \
  libfreetype6-dev libportmidi-dev python3-dev python3-numpy \
  libsdl2-dev libsdl2-image-dev libsdl2-mixer-dev libsdl2-ttf-dev

# optional pre-install hook
if [[ -f /local/pre-install.sh ]]; then
  bash /local/pre-install.sh
fi

# create runtime venv (--copies: no symlinks into the container image)
python -m venv --copies /runtime
source /runtime/bin/activate
git clone --depth 1 --branch "${PYGAME_VERSION}" \
  https://github.com/pygame-community/pygame-ce.git
cd pygame-ce && python -m pip install .
cd "$HOME"

# CardBrick's own runtime dependencies
if [[ -f /local/requirements.txt ]]; then
  python -m pip install -r /local/requirements.txt
fi

# record what went in
mkdir -p /local/build
{
  echo "python ${PYTHON_VERSION}"
  echo "pygame-ce ${PYGAME_VERSION} (built from source against Debian SDL2)"
  echo "built $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "---- pip freeze ----"
  python -m pip freeze
} > /local/build/runtime-manifest.txt

deactivate

# make the venv self-contained: interpreter shared lib + full stdlib
cp "/usr/local/lib/libpython${PY_VER}.so.1.0" /runtime/lib/
rsync -a "/usr/local/lib/python${PY_VER}/" "/runtime/lib/python${PY_VER}/"
find /runtime -name "__pycache__" -type d -exec rm -rf {} + 2>/dev/null || true

# optional post-install hook (runs right before squashing)
if [[ -f /local/post-install.sh ]]; then
  bash /local/post-install.sh
fi

# pack it
ARTIFACT="pygame-ce_${PYGAME_VERSION}_python_${PYTHON_VERSION}.squashfs"
mksquashfs /runtime "/${ARTIFACT}" -comp xz -b 1M
mv "/${ARTIFACT}" "/local/build/${ARTIFACT}"

echo "OK: /local/build/${ARTIFACT}"
