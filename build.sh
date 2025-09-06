#!/usr/bin/env bash
# Build and deploy CardBrick to a Rocknix device over SSH.
# Usage:
#   ./build.sh --host 10.0.0.159 [--user root] [--port 22]
#              [--remote-dir /storage/applications/CardBrick]
#              [--no-build] [--delete] [--dry-run] [--no-backup]
#              [--no-desktop-entry]

set -euo pipefail

# --- Defaults ---
SSH_HOST=""
SSH_USER="${USER:-root}"
SSH_PORT=22
REMOTE_DIR="/storage/applications/CardBrick"
ASSETS_DIR="./assets"
DECKS_DIR="./test_cache"
TARGET_TRIPLE="aarch64-unknown-linux-gnu"
DO_BUILD=1
DO_BACKUP=1
RSYNC_DELETE=0
DRY_RUN=0
DO_DESKTOP_ENTRY=0   # Off by default; CardBrick uses /storage/applications launcher

# --- Parse flags ---
while [[ $# -gt 0 ]]; do
  case "$1" in
    --host) SSH_HOST="$2"; shift 2 ;;
    --user) SSH_USER="$2"; shift 2 ;;
    --port) SSH_PORT="$2"; shift 2 ;;
    --remote-dir) REMOTE_DIR="$2"; shift 2 ;;
    --assets-dir) ASSETS_DIR="$2"; shift 2 ;;
    --decks-dir) DECKS_DIR="$2"; shift 2 ;;
    --target) TARGET_TRIPLE="$2"; shift 2 ;;
    --no-build) DO_BUILD=0; shift ;;
    --no-backup) DO_BACKUP=0; shift ;;
    --delete) RSYNC_DELETE=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --no-desktop-entry) DO_DESKTOP_ENTRY=0; shift ;;
    -h|--help)
      sed -n '1,60p' "$0"; exit 0 ;;
    *) echo "Unknown option: $1"; exit 2 ;;
  esac
done

if [[ -z "$SSH_HOST" ]]; then
  echo "Error: --host is required"; exit 1
fi

# --- Build (optional) ---
if [[ $DO_BUILD -eq 1 ]]; then
  echo "Building release for $TARGET_TRIPLE..."
  cross build --release --target "$TARGET_TRIPLE"
fi

BIN_PATH="./target/${TARGET_TRIPLE}/release/cardbrick"
[[ -f "$BIN_PATH" ]] || { echo "Error: binary not found at $BIN_PATH"; exit 1; }

SSH="ssh -p $SSH_PORT ${SSH_USER}@${SSH_HOST}"
RSYNC_FLAGS="-azv"
[[ $RSYNC_DELETE -eq 1 ]] && RSYNC_FLAGS="$RSYNC_FLAGS --delete"
[[ $DRY_RUN -eq 1 ]] && RSYNC_FLAGS="$RSYNC_FLAGS -n"

echo "== CardBrick deploy =="
echo "Host: ${SSH_USER}@${SSH_HOST}:${SSH_PORT}"
echo "Remote dir: $REMOTE_DIR"
echo "Binary: $BIN_PATH"
[[ $DO_BACKUP -eq 1 ]] && echo "Remote backup: enabled" || echo "Remote backup: disabled"
[[ $RSYNC_DELETE -eq 1 ]] && echo "Rsync --delete: enabled" || echo "Rsync --delete: disabled"
[[ $DRY_RUN -eq 1 ]] && echo "DRY RUN (no changes)!" || true
echo

PARENT_DIR="$(dirname "$REMOTE_DIR")"

# --- Prepare remote (ensure dir, optional backup) ---
BACKUP_CMD=':'
if [[ $DO_BACKUP -eq 1 ]]; then
  BACKUP_CMD=$(cat <<'EOS'
if [ -d "$REMOTE_DIR" ]; then
  ts="$(date +%Y%m%d-%H%M%S)"
  b="/tmp/$(basename "$REMOTE_DIR")-backup-$ts"
  echo "Backing up $REMOTE_DIR -> $b"
  cp -r "$REMOTE_DIR" "$b"
fi
EOS
)
fi

echo "Preparing remote filesystem..."
$SSH bash -s <<EOF
set -euo pipefail
REMOTE_DIR="$REMOTE_DIR"
PARENT_DIR="$PARENT_DIR"
mkdir -p "\$PARENT_DIR"
$BACKUP_CMD
mkdir -p "\$REMOTE_DIR/decks"
EOF

# --- Sync assets and data ---
echo
echo "Syncing assets and decks..."

# Assets into CardBrick app dir
rsync $RSYNC_FLAGS -e "ssh -p $SSH_PORT" "$ASSETS_DIR/icon-large.png" "${SSH_USER}@${SSH_HOST}:${REMOTE_DIR}/"
rsync $RSYNC_FLAGS -e "ssh -p $SSH_PORT" "$ASSETS_DIR/cardbrick.png" "${SSH_USER}@${SSH_HOST}:${REMOTE_DIR}/"

# gameinfo.xml goes to parent applications dir (as in prior script)
rsync $RSYNC_FLAGS -e "ssh -p $SSH_PORT" "$ASSETS_DIR/gameinfo.xml" "${SSH_USER}@${SSH_HOST}:${PARENT_DIR}/"

# Deck cache to remote decks dir
if [[ -d "$DECKS_DIR" ]]; then
  rsync $RSYNC_FLAGS -e "ssh -p $SSH_PORT" "$DECKS_DIR/" "${SSH_USER}@${SSH_HOST}:${REMOTE_DIR}/decks/"
else
  echo "Note: decks dir not found ($DECKS_DIR); skipping."
fi

# --- Copy binary and launcher ---
echo
echo "Copying binary and launcher..."
rsync $RSYNC_FLAGS -e "ssh -p $SSH_PORT" "$BIN_PATH" "${SSH_USER}@${SSH_HOST}:${REMOTE_DIR}/cardbrick"
rsync $RSYNC_FLAGS -e "ssh -p $SSH_PORT" "./CardBrick.sh" "${SSH_USER}@${SSH_HOST}:${PARENT_DIR}/"

# --- Post-copy steps ---
POST_CMDS=$(cat <<'EOS'
set -euo pipefail
echo "Post-copy steps..."
if [ -f "$REMOTE_DIR/cardbrick" ]; then
  chmod +x "$REMOTE_DIR/cardbrick"
fi
if [ -f "$PARENT_DIR/CardBrick.sh" ]; then
  chmod +x "$PARENT_DIR/CardBrick.sh"
fi

if [ "${DO_DESKTOP:-0}" = "1" ]; then
  echo "Desktop entry setup requested (Applications system)."
  # Optional: add to ES Applications system and create launcher in /storage/roms/applications
  add_apps_system() {
    local cfg="/storage/.config/emulationstation/es_systems.cfg"
    local edit_path="$cfg"
    [ -f "$edit_path" ] || return 0
    grep -q '</systemList>' "$edit_path" || return 0
    if ! grep -q '<name>applications</name>' "$edit_path"; then
      local tmp; tmp="$(mktemp)"
      awk -v block="$(cat <<'XML'
    <system>
        <name>applications</name>
        <fullname>Applications</fullname>
        <manufacturer>Custom</manufacturer>
        <release>2025</release>
        <hardware>misc</hardware>
        <path>/storage/roms/applications</path>
        <extension>.sh</extension>
        <command>%ROM%</command>
        <platform>applications</platform>
        <theme>applications</theme>
    </system>
XML
)" '
        /<\/systemList>/ && !inserted { print block; print $0; inserted=1; next }
        { print }
      ' "$edit_path" > "$tmp"
      mv "$tmp" "$edit_path"
    fi
  }

  ensure_apps_gamelist_and_launcher() {
    local app_dir="/storage/roms/applications"
    local gamelist="$app_dir/gamelist.xml"
    local launcher="$app_dir/CardBrick.sh"
    mkdir -p "$app_dir"
    [ -f "$gamelist" ] || printf '<gameList>\n</gameList>\n' > "$gamelist"
    if ! grep -Eq '<path>[[:space:]]*\./CardBrick\.sh[[:space:]]*</path>' "$gamelist"; then
      local tmp; tmp="$(mktemp)"
      awk -v block="$(cat <<'XML'
  <game>
    <path>./CardBrick.sh</path>
    <name>CardBrick</name>
    <favorite>false</favorite>
    <playcount>0</playcount>
    <lastplayed>20250820T172057</lastplayed>
    <gametime>0</gametime>
    <image>/storage/applications/CardBrick/cardbrick.png</image>
  </game>
XML
)" '
          /<\/gameList>/ && !inserted { print block; print $0; inserted=1; next }
          { print }
        ' "$gamelist" > "$tmp"
      mv "$tmp" "$gamelist"
    fi
    if [ ! -f "$launcher" ]; then
      cat > "$launcher" <<'LAUNCH'
#!/usr/bin/env bash
/storage/applications/CardBrick/CardBrick.sh
LAUNCH
      chmod +x "$launcher"
    fi
  }

  add_apps_system || true
  ensure_apps_gamelist_and_launcher || true
fi
EOS
)

if [[ $DRY_RUN -eq 1 ]]; then
  echo "DRY RUN complete (skipping remote post-copy)."
  exit 0
fi

echo
echo "Running remote post-copy steps..."
$SSH bash -s <<EOF
REMOTE_DIR="$REMOTE_DIR"
PARENT_DIR="$PARENT_DIR"
DO_DESKTOP="$DO_DESKTOP_ENTRY"
$POST_CMDS
EOF

echo "Deploy OK: $REMOTE_DIR"
