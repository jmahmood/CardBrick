#!/usr/bin/env bash
# Deploy the built CardBrick package to a Knulli device over SSH, with
# an optional on-device smoke test afterwards.
#
# Usage:
#   scripts/deploy_ssh.sh                        # root@knulli.local
#   scripts/deploy_ssh.sh --host root@192.168.1.42
#   scripts/deploy_ssh.sh --smoke                # + run smoke test after
#   scripts/deploy_ssh.sh --no-runtime           # code only, keep the
#                                                # runtime already on the
#                                                # device (fast iteration)
#
# Env:
#   CARDBRICK_DEVICE   default host (e.g. root@192.168.1.42)
#
# Knulli SSH defaults: user root, password 'linux'. For scripted use
# install a key once:  ssh-copy-id root@knulli.local
# (If sshpass is installed and SSHPASS is exported, it is used
# automatically.)

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST="$(cd "${HERE}/.." && pwd)/dist"
STAGE="${DIST}/CardBrick"
REMOTE_PORTS="/userdata/roms/ports"

HOST="${CARDBRICK_DEVICE:-root@knulli.local}"
SMOKE=0
PUSH_RUNTIME=1
while [ "$#" -gt 0 ]; do
    case "$1" in
        --host) HOST="$2"; shift ;;
        --smoke) SMOKE=1 ;;
        --no-runtime) PUSH_RUNTIME=0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

[ -d "$STAGE" ] && [ -f "${DIST}/CardBrick.sh" ] || {
    echo "ERROR: nothing staged in ${DIST} — run scripts/build_package.sh first" >&2
    exit 1
}

SSH="ssh"
if [ -n "${SSHPASS:-}" ] && command -v sshpass >/dev/null 2>&1; then
    SSH="sshpass -e ssh"
fi

run_remote() { $SSH "$HOST" "$@"; }

echo "Checking connection to ${HOST}..."
run_remote "true" || {
    echo "ERROR: cannot SSH to ${HOST}." >&2
    echo "  - Is the device on and on the network? (Knulli main menu -> Network shows the IP)" >&2
    echo "  - Try: scripts/deploy_ssh.sh --host root@<ip>   (password: linux)" >&2
    exit 1
}

run_remote "mkdir -p '${REMOTE_PORTS}/CardBrick'"

# --------------------------------------------------------- runtime file
# The squashfs is the big, rarely-changing part — skip the transfer when
# the device already has the identical file.
RUNTIME_FILE="$(ls -1 "${STAGE}/runtime/"pygame-ce_*.squashfs 2>/dev/null | head -n 1 || true)"
SKIP_RUNTIME_COPY=0
if [ -n "$RUNTIME_FILE" ] && [ "$PUSH_RUNTIME" -eq 1 ]; then
    NAME="$(basename "$RUNTIME_FILE")"
    LOCAL_SUM="$(sha256sum "$RUNTIME_FILE" | cut -d' ' -f1)"
    REMOTE_SUM="$(run_remote "sha256sum '${REMOTE_PORTS}/CardBrick/runtime/${NAME}' 2>/dev/null | cut -d' ' -f1" || true)"
    if [ -n "$REMOTE_SUM" ] && [ "$REMOTE_SUM" = "$LOCAL_SUM" ]; then
        echo "Runtime ${NAME} already on device (checksum match) — skipping upload."
        SKIP_RUNTIME_COPY=1
    fi
fi

# --------------------------------------------------------------- upload
# tar-over-ssh: works against BusyBox, no rsync needed on the device,
# preserves the executable bit regardless of FAT quirks.
echo "Uploading app$( [ "$SKIP_RUNTIME_COPY" -eq 0 ] && [ -n "$RUNTIME_FILE" ] && [ "$PUSH_RUNTIME" -eq 1 ] && echo ' + runtime' ) to ${HOST}:${REMOTE_PORTS} ..."

TAR_EXCLUDES=()
if [ "$PUSH_RUNTIME" -eq 0 ] || [ "$SKIP_RUNTIME_COPY" -eq 1 ]; then
    TAR_EXCLUDES+=(--exclude='CardBrick/runtime')
fi

( cd "$DIST" && tar cf - ${TAR_EXCLUDES[@]+"${TAR_EXCLUDES[@]}"} CardBrick.sh CardBrick ) \
    | run_remote "cd '${REMOTE_PORTS}' && tar xf -"

run_remote "chmod +x '${REMOTE_PORTS}/CardBrick.sh'"

# ---------------------------------------------------------- verification
echo "Verifying checksums on device..."
if run_remote "cd '${REMOTE_PORTS}/CardBrick' && sha256sum -c PACKAGE_MANIFEST.sha256 >/dev/null 2>&1"; then
    echo "  checksums OK"
else
    if [ "$PUSH_RUNTIME" -eq 0 ]; then
        echo "  (manifest covers the runtime too — mismatches are expected with --no-runtime)"
    else
        echo "WARNING: on-device checksum verification failed — retry the deploy." >&2
        exit 1
    fi
fi

# Refresh the Ports list so the entry appears without a reboot
# (EmulationStation HTTP API; harmless if unavailable).
run_remote "curl -s -o /dev/null http://localhost:1234/reloadgames || true" 2>/dev/null || true

if [ "$SMOKE" -eq 1 ]; then
    echo
    echo "Running on-device smoke test..."
    if run_remote "bash '${REMOTE_PORTS}/CardBrick.sh' --smoke-test"; then
        echo "SMOKE TEST: PASS"
    else
        echo "SMOKE TEST: FAIL — full log: ${HOST}:/userdata/saves/cardbrick/logs/launch.log" >&2
        exit 1
    fi
fi

echo
echo "Deployed. Launch 'CardBrick' from the Ports menu."
echo "(Not listed? Start menu -> Games Settings -> Update Gamelists.)"
