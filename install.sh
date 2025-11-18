#!/bin/bash
# CardBrick Desktop Installation Script

set -e

PACKAGE_NAME="cardbrick-desktop_0.1.0-1_amd64.deb"
PACKAGE_PATH="target/debian/$PACKAGE_NAME"

echo "CardBrick Desktop - Installation Script"
echo "========================================"
echo ""

# Check if running on supported system
if ! command -v dpkg &> /dev/null; then
    echo "Error: This script requires a Debian-based system (Ubuntu, Debian, etc.)"
    exit 1
fi

# Check if package exists
if [ ! -f "$PACKAGE_PATH" ]; then
    echo "Package not found at $PACKAGE_PATH"
    echo "Building package..."

    # Build release binary
    echo "Building release binary..."
    cargo build --release --bin cardbrick-desktop

    # Create .deb package
    echo "Creating .deb package..."
    cargo deb --no-build
fi

# Install the package
echo ""
echo "Installing $PACKAGE_NAME..."
sudo dpkg -i "$PACKAGE_PATH"

# Fix any dependency issues
echo ""
echo "Checking dependencies..."
sudo apt-get install -f -y

echo ""
echo "Installation complete!"
echo ""
echo "You can now run CardBrick Desktop:"
echo "  - From command line: cardbrick-desktop --deck /path/to/deck.cbdeck.json"
echo "  - From application menu: Search for 'CardBrick Desktop'"
echo ""
