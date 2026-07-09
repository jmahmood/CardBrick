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
#   scripts/deploy_ssh.sh --full                 # skip the manifest diff
#                                                # and re-upload the whole
#                                                # bundle
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
FULL=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --host) HOST="$2"; shift ;;
        --smoke) SMOKE=1 ;;
        --no-runtime) PUSH_RUNTIME=0 ;;
        --full) FULL=1 ;;
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
#
# The bundle carries thousands of media files that almost never change,
# and re-extracting all of them onto the SD card dominates deploy time.
# The previous deploy left its PACKAGE_MANIFEST.sha256 (per-file SHA-256)
# on the device: diff it against the freshly built one and send only the
# new/changed files, deleting remotely whatever the build dropped. The
# final on-device `sha256sum -c` still verifies the WHOLE bundle, so a
# device tree that drifted out of sync with its manifest fails loudly
# rather than silently — rerun with --full to resend everything.
TMPD="$(mktemp -d)"
trap 'rm -rf "$TMPD"' EXIT
CHANGED_LIST="${TMPD}/changed"
DELETED_LIST="${TMPD}/deleted"

INCREMENTAL=0
if [ "$FULL" -eq 0 ]; then
    if run_remote "cat '${REMOTE_PORTS}/CardBrick/PACKAGE_MANIFEST.sha256' 2>/dev/null" \
            > "${TMPD}/remote-manifest" 2>/dev/null && [ -s "${TMPD}/remote-manifest" ]; then
        INCREMENTAL=1
        # Manifest lines are "<sha256>  ./<path>". A file counts as
        # changed when its exact line (hash+path) is absent remotely.
        awk 'NR==FNR { seen[$0]=1; next }
             !($0 in seen) { p=$0; sub(/^[^ ]+  \.\//, "", p); print p }' \
            "${TMPD}/remote-manifest" "${STAGE}/PACKAGE_MANIFEST.sha256" > "$CHANGED_LIST"
        awk 'NR==FNR { p=$0; sub(/^[^ ]+  \.\//, "", p); have[p]=1; next }
             { p=$0; sub(/^[^ ]+  \.\//, "", p); if (!(p in have)) print p }' \
            "${STAGE}/PACKAGE_MANIFEST.sha256" "${TMPD}/remote-manifest" > "$DELETED_LIST"
        # Runtime is handled by its own checksum-skip above; keep it out
        # of the diff when it isn't being pushed, and never delete it on
        # the device just because this stage was built --no-runtime.
        if [ "$PUSH_RUNTIME" -eq 0 ] || [ "$SKIP_RUNTIME_COPY" -eq 1 ]; then
            grep -v '^runtime/' "$CHANGED_LIST" > "${CHANGED_LIST}.f" || true
            mv "${CHANGED_LIST}.f" "$CHANGED_LIST"
        fi
        if [ ! -d "${STAGE}/runtime" ]; then
            grep -v '^runtime/' "$DELETED_LIST" > "${DELETED_LIST}.f" || true
            mv "${DELETED_LIST}.f" "$DELETED_LIST"
        fi
    fi
fi

# COPYFILE_DISABLE stops macOS tar from adding AppleDouble (._*) entries
# and xattr headers the device's BusyBox tar would dump as junk files;
# 'o' on extract skips chown — /userdata is exFAT, chown fails there and
# makes tar exit non-zero, killing the deploy under set -e.
if [ "$INCREMENTAL" -eq 1 ]; then
    TOTAL="$(wc -l < "${STAGE}/PACKAGE_MANIFEST.sha256" | tr -d ' ')"
    N_CHANGED="$(wc -l < "$CHANGED_LIST" | tr -d ' ')"
    N_DELETED="$(wc -l < "$DELETED_LIST" | tr -d ' ')"
    echo "Incremental upload to ${HOST}:${REMOTE_PORTS}: ${N_CHANGED} changed, ${N_DELETED} removed (of ${TOTAL} files)"
    # NUL-separated -T list: the manifest is not listed in itself, so add
    # it explicitly — the next deploy diffs against it.
    { echo "CardBrick.sh"
      echo "CardBrick/PACKAGE_MANIFEST.sha256"
      sed 's|^|CardBrick/|' "$CHANGED_LIST"
    } | tr '\n' '\0' > "${TMPD}/tarlist"
    ( cd "$DIST" && COPYFILE_DISABLE=1 tar cf - --null -T "${TMPD}/tarlist" ) \
        | run_remote "cd '${REMOTE_PORTS}' && tar xof -"
    if [ -s "$DELETED_LIST" ]; then
        tr '\n' '\0' < "$DELETED_LIST" \
            | run_remote "cd '${REMOTE_PORTS}/CardBrick' && xargs -0 rm -f"
    fi
else
    echo "Uploading app$( [ "$SKIP_RUNTIME_COPY" -eq 0 ] && [ -n "$RUNTIME_FILE" ] && [ "$PUSH_RUNTIME" -eq 1 ] && echo ' + runtime' ) to ${HOST}:${REMOTE_PORTS} ..."
    TAR_EXCLUDES=()
    if [ "$PUSH_RUNTIME" -eq 0 ] || [ "$SKIP_RUNTIME_COPY" -eq 1 ]; then
        TAR_EXCLUDES+=(--exclude='CardBrick/runtime')
    fi
    ( cd "$DIST" && COPYFILE_DISABLE=1 tar cf - \
          --exclude='._*' --exclude='.DS_Store' \
          ${TAR_EXCLUDES[@]+"${TAR_EXCLUDES[@]}"} CardBrick.sh CardBrick ) \
        | run_remote "cd '${REMOTE_PORTS}' && tar xof -"
fi

# Sweep out ._* junk left behind by deploys made before the fix above.
run_remote "rm -f '${REMOTE_PORTS}/._CardBrick.sh' '${REMOTE_PORTS}/._CardBrick'; \
    find '${REMOTE_PORTS}/CardBrick' -name '._*' -exec rm -f {} + 2>/dev/null || true"

run_remote "chmod +x '${REMOTE_PORTS}/CardBrick.sh'"

# ---------------------------------------------------------- verification
echo "Verifying checksums on device..."
if run_remote "cd '${REMOTE_PORTS}/CardBrick' && sha256sum -c PACKAGE_MANIFEST.sha256 >/dev/null 2>&1"; then
    echo "  checksums OK"
else
    if [ "$PUSH_RUNTIME" -eq 0 ]; then
        echo "  (manifest covers the runtime too — mismatches are expected with --no-runtime)"
    else
        echo "WARNING: on-device checksum verification failed — retry with --full to resend the whole bundle." >&2
        exit 1
    fi
fi

# ----------------------------------------------------- gamelist metadata
# A bare .sh in Ports shows no description or image. Merge the metadata
# from assets/gameinfo.xml (repo root) into the device's gamelist.xml,
# preserving ES-owned play stats (playcount / lastplayed / gametime).
# Best-effort: a failure here never fails the deploy.
REPO_ROOT="$(cd "${HERE}/../../.." && pwd)"
GAMEINFO="${REPO_ROOT}/assets/gameinfo.xml"
if [ -f "$GAMEINFO" ] && command -v python3 >/dev/null 2>&1; then
    echo "Updating Ports gamelist metadata..."
    # Stage predates icon packaging? Push the image directly.
    if [ ! -f "${STAGE}/icon-large.png" ] && [ -f "${REPO_ROOT}/assets/icon-large.png" ]; then
        run_remote "cat > '${REMOTE_PORTS}/CardBrick/icon-large.png'" \
            < "${REPO_ROOT}/assets/icon-large.png" || true
    fi
    GL_OLD="$(mktemp)"; GL_NEW="$(mktemp)"
    run_remote "cat '${REMOTE_PORTS}/gamelist.xml' 2>/dev/null" > "$GL_OLD" || true
    if python3 "${HERE}/merge_gamelist.py" "$GAMEINFO" "$GL_OLD" > "$GL_NEW"; then
        run_remote "cat > '${REMOTE_PORTS}/.gamelist.xml.cardbrick' \
            && mv '${REMOTE_PORTS}/.gamelist.xml.cardbrick' '${REMOTE_PORTS}/gamelist.xml'" \
            < "$GL_NEW" || echo "WARNING: could not write gamelist.xml on device" >&2
    else
        echo "WARNING: gamelist merge failed — Ports entry will lack desc/image" >&2
    fi
    rm -f "$GL_OLD" "$GL_NEW"
fi

# Refresh the Ports list so the entry (and new metadata) appears without
# a reboot (EmulationStation HTTP API; harmless if unavailable).
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
