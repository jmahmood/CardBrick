#!/bin/bash
# CardBrick Sync Daemon Setup Script
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DAEMON_USER="cardbrick-sync"
DATA_DIR="/var/lib/cardbrick"
CONFIG_DIR="/etc/cardbrick"
INSTALL_DIR="/usr/local/bin"
SERVICE_FILE="/etc/systemd/system/cardbrick-syncd.service"

echo "Setting up CardBrick Sync Daemon..."

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo "This script must be run as root" 
   exit 1
fi

# Create system user and group
if ! id "$DAEMON_USER" &>/dev/null; then
    echo "Creating system user: $DAEMON_USER"
    useradd --system --shell /usr/sbin/nologin --home-dir "$DATA_DIR" --create-home "$DAEMON_USER"
else
    echo "User $DAEMON_USER already exists"
fi

# Create directories with proper permissions
echo "Creating directories..."
mkdir -p "$DATA_DIR"/{inbox,keys/devices}
mkdir -p "$CONFIG_DIR"
chown -R "$DAEMON_USER:$DAEMON_USER" "$DATA_DIR"
chmod 750 "$DATA_DIR"
chmod 700 "$DATA_DIR/keys"

# Install Python dependencies
echo "Installing Python dependencies..."
if ! command -v pip3 &> /dev/null; then
    echo "Error: pip3 not found. Please install Python 3 and pip."
    exit 1
fi

pip3 install -r "$SCRIPT_DIR/requirements.txt"

# Create wrapper script
echo "Creating daemon wrapper script..."
cat > "$INSTALL_DIR/cardbrick-syncd" << 'EOF'
#!/usr/bin/env python3
import sys
import os

# Add the daemon directory to Python path
daemon_dir = os.path.join(os.path.dirname(__file__), '..', 'lib', 'cardbrick-syncd')
if os.path.exists(daemon_dir):
    sys.path.insert(0, daemon_dir)

# Import and run the daemon
try:
    from cardbrick_syncd.main import main
    import asyncio
    asyncio.run(main())
except ImportError as e:
    print(f"Error importing daemon: {e}")
    sys.exit(1)
except Exception as e:
    print(f"Daemon error: {e}")
    sys.exit(1)
EOF

chmod +x "$INSTALL_DIR/cardbrick-syncd"

# Copy daemon source code
echo "Installing daemon source..."
DAEMON_LIB_DIR="/usr/local/lib/cardbrick-syncd"
mkdir -p "$DAEMON_LIB_DIR"
cp -r "$SCRIPT_DIR"/*.py "$DAEMON_LIB_DIR/"
chown -R root:root "$DAEMON_LIB_DIR"
chmod -R 644 "$DAEMON_LIB_DIR"/*.py

# Install systemd service
echo "Installing systemd service..."
cp "$SCRIPT_DIR/cardbrick-syncd.service" "$SERVICE_FILE"
systemctl daemon-reload

# Generate SSH keys if they don't exist
echo "Setting up SSH keys..."
SSH_KEY_PATH="$DATA_DIR/id_ed25519"
if [[ ! -f "$SSH_KEY_PATH" ]]; then
    sudo -u "$DAEMON_USER" ssh-keygen -t ed25519 -f "$SSH_KEY_PATH" -N "" -C "cardbrick-sync@$(hostname)"
    echo "Generated SSH key: $SSH_KEY_PATH"
else
    echo "SSH key already exists: $SSH_KEY_PATH"
fi

# Create known_hosts file
KNOWN_HOSTS_PATH="$DATA_DIR/known_hosts"
if [[ ! -f "$KNOWN_HOSTS_PATH" ]]; then
    sudo -u "$DAEMON_USER" touch "$KNOWN_HOSTS_PATH"
    chmod 600 "$KNOWN_HOSTS_PATH"
    echo "Created known_hosts file: $KNOWN_HOSTS_PATH"
fi

# Set up log rotation
echo "Setting up log rotation..."
cat > /etc/logrotate.d/cardbrick-syncd << 'EOF'
/var/log/cardbrick/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 cardbrick-sync cardbrick-sync
    postrotate
        systemctl reload cardbrick-syncd.service > /dev/null 2>&1 || true
    endscript
}
EOF

# Enable and start service
echo "Enabling service..."
systemctl enable cardbrick-syncd.service

echo "Setup complete!"
echo ""
echo "Next steps:"
echo "1. Add handheld device public keys to: $DATA_DIR/keys/devices/"
echo "2. Add desktop SSH key to handheld authorized_keys"
echo "3. Start the service: systemctl start cardbrick-syncd.service"
echo "4. Check status: systemctl status cardbrick-syncd.service"
echo ""
echo "Desktop public key for handheld setup:"
cat "$SSH_KEY_PATH.pub"