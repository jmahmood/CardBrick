#!/bin/sh
# Rootless installation for a prepared Linux account.
set -eu

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(CDPATH= cd -- "$HERE/../.." && pwd)"
APP_DIR="${CARDBRICK_SERVER_APP:-$HOME/.local/share/cardbrick-sync}"
DATA_DIR="${CARDBRICK_SERVER_ROOT:-$HOME/cardbrick-data}"
UNIT_DIR="$HOME/.config/systemd/user"

mkdir -p "$APP_DIR" "$DATA_DIR" "$HOME/.local/bin" "$UNIT_DIR"
rm -rf "$APP_DIR/cardbrick_server"
cp -R "$REPO_ROOT/cardbrick_server" "$APP_DIR/"
install -m 0755 "$HERE/cardbrick-server-user" \
    "$HOME/.local/bin/cardbrick-server"
install -m 0644 "$HERE/cardbrick-sync-server-user.service" \
    "$UNIT_DIR/cardbrick-sync-server.service"

systemctl --user daemon-reload
systemctl --user enable --now cardbrick-sync-server.service

echo "CardBrick server installed for $(id -un)."
echo "Storage: $DATA_DIR"
echo "Admin:   $HOME/.local/bin/cardbrick-server --help"
echo "Health:  http://$(hostname -I | awk '{print $1}'):6429/health"
