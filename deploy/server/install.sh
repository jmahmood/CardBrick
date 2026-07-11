#!/bin/sh
# Install the dependency-free CardBrick sync server on Debian/Raspberry Pi OS.
set -eu

[ "$(id -u)" -eq 0 ] || {
    echo "Run this installer as root." >&2
    exit 1
}

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(CDPATH= cd -- "$HERE/../.." && pwd)"

if ! id cardbrick >/dev/null 2>&1; then
    useradd --system --home /srv/cardbrick --shell /usr/sbin/nologin cardbrick
fi

mkdir -p /opt/cardbrick /srv/cardbrick
rm -rf /opt/cardbrick/cardbrick_server
cp -R "$REPO_ROOT/cardbrick_server" /opt/cardbrick/
chown -R root:root /opt/cardbrick
chown -R cardbrick:cardbrick /srv/cardbrick

install -m 0644 "$HERE/cardbrick-sync-server.service" \
    /etc/systemd/system/cardbrick-sync-server.service
install -m 0755 "$HERE/cardbrick-server" /usr/local/bin/cardbrick-server

if [ -d /etc/avahi/services ]; then
    install -m 0644 "$HERE/cardbrick.service" \
        /etc/avahi/services/cardbrick.service
fi

systemctl daemon-reload
systemctl enable --now cardbrick-sync-server.service

echo "CardBrick server installed."
echo "Storage: /srv/cardbrick"
echo "Admin:   cardbrick-server --help"
echo "Health:  http://$(hostname).local:6429/health"
