#!/usr/bin/env bash
# Deploy the built PortMaster package over SSH.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PORTMASTER_DIR="$(cd "$HERE/.." && pwd)"
STAGE="$PORTMASTER_DIR/dist/cardbrick"

HOST="${CARDBRICK_DEVICE:-}"
REMOTE_PORTS="${PORTMASTER_PORTS:-}"
RUN_SMOKE=0
PORT_NAME="cardbrick"
LAUNCHER_NAME="CardBrick.sh"
REPLACE=0

usage() {
    cat <<'EOF'
Usage: scripts/deploy_ssh.sh --host user@device [options]

Options:
  --host HOST          SSH destination; defaults to CARDBRICK_DEVICE
  --ports PATH         Remote PortMaster ports directory
  --parallel           Install as CardBrickPM/cardbrickpm beside CardBrick
  --replace            Allow replacement of an existing matching install
  --smoke              Run CardBrick --smoke-test after deployment
  -h, --help           Show this help

The default remote ports directory is detected from PortMaster's control.txt
on the target. Existing cardbrick/conf data is preserved during replacement.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --host)
            [ "$#" -ge 2 ] || { echo "ERROR: --host requires a value" >&2; exit 2; }
            HOST="$2"
            shift
            ;;
        --ports)
            [ "$#" -ge 2 ] || { echo "ERROR: --ports requires a value" >&2; exit 2; }
            REMOTE_PORTS="$2"
            shift
            ;;
        --parallel)
            PORT_NAME="cardbrickpm"
            LAUNCHER_NAME="CardBrickPM.sh"
            ;;
        --replace) REPLACE=1 ;;
        --smoke) RUN_SMOKE=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "ERROR: unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

[ -n "$HOST" ] || { echo "ERROR: pass --host user@device" >&2; exit 2; }
[ -d "$STAGE" ] || {
    echo "ERROR: package stage missing: $STAGE" >&2
    echo "Build it first with: make -C deploy/portmaster package" >&2
    exit 1
}
[ -f "$STAGE/CardBrick.sh" ] || { echo "ERROR: launcher missing from package" >&2; exit 1; }
[ -x "$STAGE/cardbrick/runtime/bin/python3" ] || {
    echo "ERROR: ARM64 runtime missing from package" >&2
    exit 1
}

command -v ssh >/dev/null 2>&1 || { echo "ERROR: ssh is required" >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { echo "ERROR: tar is required" >&2; exit 1; }

if [ -z "$REMOTE_PORTS" ]; then
    echo "Detecting PortMaster ports directory on $HOST..."
    REMOTE_PORTS="$(ssh "$HOST" 'if [ -f /userdata/roms/ports/PortMaster/control.txt ]; then
        echo "PortMaster control: /userdata/roms/ports/PortMaster/control.txt" >&2
        printf "/userdata/roms/ports\\n"
    fi')"
fi

if [ -z "$REMOTE_PORTS" ]; then
    REMOTE_PORTS="$(ssh "$HOST" bash -s <<'REMOTE'
set -e
XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
controlfolder=""
for folder in \
    /userdata/roms/ports/PortMaster \
    /userdata/system/.local/share/PortMaster \
    /opt/system/Tools/PortMaster \
    /opt/tools/PortMaster \
    "$XDG_DATA_HOME/PortMaster" \
    /roms/ports/PortMaster \
    /roms/ports/PortMaster/PortMaster; do
    if [ -f "$folder/control.txt" ]; then
        controlfolder="$folder"
        break
    fi
done

if [ -z "$controlfolder" ]; then
    candidate="$(find / -maxdepth 6 -type f -iname control.txt \
        -path '*[Pp]ort[Mm]aster*' -print -quit 2>/dev/null || true)"
    [ -z "$candidate" ] || controlfolder="$(dirname "$candidate")"
fi

[ -n "$controlfolder" ] && [ -f "$controlfolder/control.txt" ] || {
    echo "PortMaster control.txt was not found; pass --ports /path/to/ports" >&2
    exit 1
}

echo "PortMaster control: $controlfolder/control.txt" >&2

directory="$(grep -m 1 -E '^[[:space:]]*directory[[:space:]]*=' \
    "$controlfolder/control.txt" | cut -d= -f2- | cut -d'#' -f1 | tr -d '"[:space:]')"
[ -n "$directory" ] || {
    echo "PortMaster control.txt does not define a ports directory" >&2
    exit 1
}
case "$controlfolder" in
    /userdata/roms/ports/PortMaster)
        directory="userdata/roms"
        ;;
esac
printf '/%s/ports\n' "$directory"
REMOTE
    )"
fi

[ -n "$REMOTE_PORTS" ] || { echo "ERROR: remote ports directory is empty" >&2; exit 1; }
echo "Deploying CardBrick to $HOST:$REMOTE_PORTS"

case "$REMOTE_PORTS" in
    *"'"*|*";"*|*"&"*|*"|"*|*" "*)
        echo "ERROR: remote ports path contains unsupported shell characters: $REMOTE_PORTS" >&2
        exit 1
        ;;
esac

REMOTE_STAGE="$REMOTE_PORTS/.cardbrick-stage-$(date +%s)-$$"

DEPLOY_TREE="$STAGE"
TEMP_TREE=""
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

if [ "$PORT_NAME" != "cardbrick" ] || [ "$LAUNCHER_NAME" != "CardBrick.sh" ]; then
    TEMP_TREE="$(mktemp -d)"
    trap 'rm -rf "$TEMP_TREE"' EXIT
    cp -a "$STAGE/." "$TEMP_TREE/"
    mv "$TEMP_TREE/cardbrick" "$TEMP_TREE/$PORT_NAME"
    mv "$TEMP_TREE/CardBrick.sh" "$TEMP_TREE/$LAUNCHER_NAME"
    write_manifest "$TEMP_TREE" "$TEMP_TREE/$PORT_NAME/PACKAGE_MANIFEST.sha256" \
        "$LAUNCHER_NAME" "$PORT_NAME"
    DEPLOY_TREE="$TEMP_TREE"
fi

# Stream only the installable items into a remote staging directory first.
# The separate finalize step below avoids mixing a tar stream with a shell
# heredoc on the same SSH stdin.
(
    cd "$DEPLOY_TREE"
    COPYFILE_DISABLE=1 tar --format=ustar --no-xattrs -cf - --exclude='._*' \
        "$LAUNCHER_NAME" "$PORT_NAME"
) | \
    ssh "$HOST" "mkdir -p '$REMOTE_STAGE' && tar -xof - -C '$REMOTE_STAGE'"

# Replace only after the complete staged tree is present. The previous files
# remain in a sibling backup until the new manifest verifies, so a failed
# update rolls back instead of stranding the existing installation or data.
ssh "$HOST" bash -s -- "$REMOTE_PORTS" "$REMOTE_STAGE" "$PORT_NAME" "$LAUNCHER_NAME" "$REPLACE" <<'REMOTE'
set -u
ports="$1"
stage="$2"
port_name="$3"
launcher_name="$4"
replace="$5"
backup="$ports/.${port_name}.previous-$(date +%s)-$$"
had_previous=0

rollback() {
    echo "deployment failed; restoring the previous CardBrick install" >&2
    rm -rf "$ports/$port_name"
    rm -f "$ports/$launcher_name"
    [ ! -e "$backup/$port_name" ] || mv "$backup/$port_name" "$ports/$port_name"
    [ ! -e "$backup/$launcher_name" ] || mv "$backup/$launcher_name" "$ports/$launcher_name"
    rm -rf "$stage"
}

[ -f "$stage/$port_name/runtime/bin/python3" ] || {
    echo "staged runtime interpreter is missing" >&2
    rm -rf "$stage"
    exit 1
}

if [ "$replace" != "1" ] && {
    [ -e "$ports/$port_name" ] || [ -e "$ports/$launcher_name" ];
}; then
    echo "matching CardBrick install already exists; use --parallel or --replace" >&2
    rm -rf "$stage"
    exit 1
fi

if [ -e "$ports/$port_name" ] || [ -e "$ports/$launcher_name" ]; then
    had_previous=1
fi

if [ -d "$ports/$port_name/conf" ]; then
    mkdir -p "$stage/$port_name"
    cp -a "$ports/$port_name/conf" "$stage/$port_name/" || {
        echo "could not copy existing CardBrick data into the staged install" >&2
        rm -rf "$stage"
        exit 1
    }
fi

if [ "$had_previous" -eq 1 ]; then
    mkdir -p "$backup" || { rm -rf "$stage"; exit 1; }
    [ ! -e "$ports/$port_name" ] || mv "$ports/$port_name" "$backup/$port_name" || {
        rm -rf "$stage" "$backup"
        exit 1
    }
    [ ! -e "$ports/$launcher_name" ] || mv "$ports/$launcher_name" "$backup/$launcher_name" || {
        rollback
        exit 1
    }
fi

mv "$stage/$port_name" "$ports/$port_name" || { rollback; exit 1; }
mv "$stage/$launcher_name" "$ports/$launcher_name" || { rollback; exit 1; }
chmod +x "$ports/$launcher_name" "$ports/$port_name/runtime/bin/python3" 2>/dev/null || true

if command -v sha256sum >/dev/null 2>&1 && \
   ! (cd "$ports" && sha256sum -c "$port_name/PACKAGE_MANIFEST.sha256" >/dev/null 2>&1); then
    echo "new package checksum verification failed" >&2
    rollback
    exit 1
fi

rm -rf "$backup"
rm -rf "$stage"
sync
echo "CardBrick deployed successfully"
REMOTE

if [ "$RUN_SMOKE" -eq 1 ]; then
    echo "Running remote CardBrick smoke test..."
    ssh "$HOST" bash -s -- "$REMOTE_PORTS" "$LAUNCHER_NAME" <<'REMOTE'
set -e
cd "$1"
./"$2" --smoke-test
REMOTE
fi

echo "Done. Launch CardBrick from the PortMaster/Ports menu."
