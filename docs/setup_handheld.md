# CardBrick Handheld Setup Guide

This guide walks you through setting up CardBrick sync between your desktop and handheld device.

## Overview

CardBrick uses a "doorbell-pull" sync system:
1. 🔔 **Handheld rings**: Device discovers desktop and sends sync request  
2. 🖥️ **Desktop pulls**: Authenticated rsync operation transfers decks
3. ✅ **Status update**: Desktop confirms sync completion

## Prerequisites

- Desktop running Linux with Python 3.8+
- Handheld device (TrimUI Brick/RG35XX Plus) with CardBrick installed
- Both devices on the same network

## Step 1: Desktop Setup

### Install the Sync Daemon

```bash
# Clone CardBrick repository
git clone https://github.com/CardBrick/CardBrick.git
cd CardBrick

# Install Python dependencies
pip install -r cardbrick-syncd/requirements.txt
pip install -r cardbrick-cli/requirements.txt

# Run setup script (requires root)
sudo cardbrick-syncd/setup.sh
```

This will:
- Create `cardbrick-sync` system user
- Install systemd service with security hardening
- Generate SSH keys for secure sync
- Set up directory structure in `/var/lib/cardbrick/`

### Start the Service

```bash
# Start and enable the daemon
sudo systemctl start cardbrick-syncd.service
sudo systemctl enable cardbrick-syncd.service

# Check status
sudo systemctl status cardbrick-syncd.service
```

The daemon will:
- Listen on port 6429 for doorbell requests
- Advertise `_cardbrick._tcp` service via Avahi/mDNS
- Provide web UI at `http://localhost:6429/docs`

## Step 2: SSH Key Exchange

### Get Desktop Public Key

```bash
# Display desktop's public key
sudo cat /var/lib/cardbrick/id_ed25519.pub
```

Copy this key - you'll need it for the handheld setup.

### Add Key to Handheld (via QR Code)

1. Generate QR code for easy transfer:
```bash
# Install qrencode if needed
sudo apt install qrencode

# Generate QR code of public key
sudo cat /var/lib/cardbrick/id_ed25519.pub | qrencode -t UTF8
```

2. On handheld, scan QR code and add to authorized_keys:
```bash
# On handheld device
mkdir -p ~/.ssh
echo "PUBLIC_KEY_FROM_QR" >> ~/.ssh/authorized_keys
chmod 600 ~/.ssh/authorized_keys
```

### Add Handheld Key to Desktop

1. Get handheld's public key:
```bash
# On handheld
cat ~/.ssh/id_ed25519.pub
```

2. Add to desktop's known devices:
```bash
# On desktop - replace HANDHELD_KEY with actual key
sudo -u cardbrick-sync mkdir -p /var/lib/cardbrick/keys/devices
echo "HANDHELD_KEY" | sudo -u cardbrick-sync tee /var/lib/cardbrick/keys/devices/handheld01.pem
```

## Step 3: Handheld Setup

### Build Sync Client

```bash
# On development machine (cross-compile for ARM64)
cross build --release --target aarch64-unknown-linux-gnu --bin sync_ring

# Copy to handheld device
scp target/aarch64-unknown-linux-gnu/release/sync_ring root@HANDHELD_IP:/usr/local/bin/
```

### Configure Handheld Device

1. Create CardBrick user:
```bash
# On handheld
useradd --system --shell /bin/bash cardbrick
mkdir -p /home/cardbrick/.ssh
chown cardbrick:cardbrick /home/cardbrick/.ssh
```

2. Set up SSH forced command (security):
```bash
# Create sync wrapper script
cat > /usr/local/bin/cardbrick-sync-shell << 'EOF'
#!/bin/bash
# Forced command for CardBrick sync
case "$SSH_ORIGINAL_COMMAND" in
    rsync\ --server*)
        exec $SSH_ORIGINAL_COMMAND
        ;;
    *)
        echo "Only rsync commands allowed"
        exit 1
        ;;
esac
EOF

chmod +x /usr/local/bin/cardbrick-sync-shell

# Update authorized_keys with forced command
echo 'command="/usr/local/bin/cardbrick-sync-shell",no-port-forwarding,no-X11-forwarding,no-agent-forwarding DESKTOP_PUBLIC_KEY' > /home/cardbrick/.ssh/authorized_keys
```

3. Create deck directory:
```bash
mkdir -p /flash/decks
chown cardbrick:cardbrick /flash/decks
```

## Step 4: Test the Setup

### Manual Sync Test

```bash
# On handheld, trigger sync
/usr/local/bin/sync_ring

# Check output - should show:
# "Found CardBrick service at DESKTOP_IP:6429"
# "Sync request successful: accepted"
```

### Desktop Monitoring

```bash
# Check daemon logs
sudo journalctl -u cardbrick-syncd.service -f

# Check sync status via API
curl http://localhost:6429/status

# View web interface
firefox http://localhost:6429/docs
```

## Step 5: Import and Sync Decks

### Import Decks on Desktop

```bash
# Import CSV file
cardbrick-cli import flashcards.csv --auto-sync

# Import Markdown file  
cardbrick-cli import study_notes.md --deck-name "Study Notes" --auto-sync

# Check import status
cardbrick-cli status
```

### Trigger Sync from Handheld

The handheld sync can be triggered:

1. **Manually**: Run `/usr/local/bin/sync_ring`
2. **UI Integration**: Add "🔔 Sync" button to CardBrick main menu
3. **Scheduled**: Add to cron for automatic sync

### Verify Sync

```bash
# On handheld, check received decks
ls -la /flash/decks/

# On desktop, check sync logs
sudo journalctl -u cardbrick-syncd.service | grep "Sync completed"
```

## Troubleshooting

### Common Issues

**"No CardBrick services found"**
- Check both devices are on same network
- Verify desktop daemon is running: `systemctl status cardbrick-syncd`
- Test mDNS: `avahi-browse -rt _cardbrick._tcp`

**"SSH connection failed"**
- Verify SSH keys are properly exchanged
- Check SSH permissions (600 for keys, 700 for .ssh)
- Test manual SSH: `ssh cardbrick@DESKTOP_IP`

**"Rate limited"**  
- Handheld enforces 15-minute rate limit
- Wait or check `sync_ring` logs for next allowed time

**"Signature validation failed"**
- Ensure handheld device key is added to desktop
- Check system time sync between devices
- Verify key fingerprints match

### Debug Commands

```bash
# Desktop debugging
sudo journalctl -u cardbrick-syncd.service -n 50
curl http://localhost:6429/health
curl http://localhost:6429/auth/stats

# Handheld debugging  
/usr/local/bin/sync_ring --verbose
ssh -v cardbrick@DESKTOP_IP
```

### Log Files

- **Desktop**: `journalctl -u cardbrick-syncd.service`
- **Handheld**: `/tmp/cardbrick_sync_state.json`
- **Network**: `tcpdump -i any port 6429`

## Security Notes

- All communication uses ed25519 cryptographic signatures
- SSH connections are restricted to rsync commands only
- Desktop daemon runs with minimal privileges (systemd hardening)
- Rate limiting prevents abuse (max 1 sync per 15 minutes)
- Keys are stored with secure permissions (600/700)

## Advanced Configuration

### Custom Network Settings

```bash
# Change daemon port (default 6429)
sudo systemctl edit cardbrick-syncd.service

# Add override:
[Service]
Environment="CARDBRICK_HTTP_PORT=8080"
```

### Multiple Handheld Devices

```bash
# Add additional device keys
sudo -u cardbrick-sync tee /var/lib/cardbrick/keys/devices/handheld02.pem
sudo -u cardbrick-sync tee /var/lib/cardbrick/keys/devices/handheld03.pem
```

### Automated Imports

```bash
# Watch directory for new files
inotifywait -m /path/to/imports -e create --format '%f' | while read file; do
    cardbrick-cli import "/path/to/imports/$file" --auto-sync
done
```

---

**🎉 Setup Complete!** 

You now have secure, authenticated sync between your desktop and handheld CardBrick devices. Tap "🔔 Sync" on your handheld to pull the latest decks!