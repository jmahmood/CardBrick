#!/bin/bash
# CardBrick Sync Daemon Setup Script
# Creates virtual environment and installs dependencies

set -e  # Exit on any error

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/venv-syncd"
DAEMON_DIR="$SCRIPT_DIR/cardbrick-syncd"
CLI_DIR="$SCRIPT_DIR/cardbrick-cli"
DATA_DIR="${CARDBRICK_DATA_DIR:-/var/lib/cardbrick}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
check_directory() {
    if [[ ! -d "$DAEMON_DIR" ]]; then
        log_error "cardbrick-syncd directory not found!"
        log_error "Please run this script from the CardBrick root directory"
        exit 1
    fi
    
    if [[ ! -f "$DAEMON_DIR/main.py" ]]; then
        log_error "main.py not found in cardbrick-syncd directory!"
        exit 1
    fi
    
    log_success "Found CardBrick daemon directory"
}

# Check system dependencies
check_system_deps() {
    log_info "Checking system dependencies..."
    
    # Check Python
    if ! command -v python3 &> /dev/null; then
        log_error "Python 3 is required but not installed"
        exit 1
    fi
    
    PYTHON_VERSION=$(python3 -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')
    log_info "Found Python $PYTHON_VERSION"
    
    # Check if python3-venv is available
    if ! python3 -c "import venv" 2>/dev/null; then
        log_warning "python3-venv module not found"
        log_info "Installing python3-venv..."
        
        if command -v apt-get &> /dev/null; then
            sudo apt-get update && sudo apt-get install -y python3-venv python3-dev
        elif command -v yum &> /dev/null; then
            sudo yum install -y python3-venv python3-devel
        else
            log_error "Please install python3-venv package for your system"
            exit 1
        fi
    fi
    
    # Check for Avahi development packages (optional but recommended)
    if command -v apt-get &> /dev/null; then
        log_info "Checking for Avahi packages for mDNS support..."
        if ! dpkg -l | grep -q python3-dbus; then
            log_info "Installing python3-dbus for Avahi support..."
            sudo apt-get install -y python3-dbus libdbus-1-dev || log_warning "Could not install dbus packages"
        fi
        
        if ! dpkg -l | grep -q libavahi-compat-libdnssd-dev; then
            log_info "Installing Avahi development packages..."
            sudo apt-get install -y avahi-daemon libavahi-compat-libdnssd-dev || log_warning "Could not install Avahi packages"
        fi
    elif command -v yum &> /dev/null; then
        log_info "Installing Avahi packages for RHEL/CentOS..."
        sudo yum install -y avahi avahi-devel dbus-python || log_warning "Could not install Avahi packages"
    fi
    
    log_info "Note: mDNS will work without Avahi, using simplified discovery"
}

# Create virtual environment
create_venv() {
    if [[ -d "$VENV_DIR" ]]; then
        log_warning "Virtual environment already exists at $VENV_DIR"
        read -p "Recreate it? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            log_info "Removing existing virtual environment..."
            rm -rf "$VENV_DIR"
        else
            log_info "Using existing virtual environment"
            return 0
        fi
    fi
    
    log_info "Creating virtual environment at $VENV_DIR..."
    python3 -m venv --system-site-packages "$VENV_DIR"
    
    # Update pip in virtual environment
    log_info "Upgrading pip in virtual environment..."
    "$VENV_DIR/bin/python" -m pip install --upgrade pip
    
    log_success "Virtual environment created successfully"
}

# Install Python dependencies
install_dependencies() {
    log_info "Installing Python dependencies..."
    
    # Install daemon dependencies
    log_info "Installing cardbrick-syncd dependencies..."
    "$VENV_DIR/bin/pip" install -r "$DAEMON_DIR/requirements.txt"
    
    # Install CLI dependencies (for wormhole functionality)
    if [[ -f "$CLI_DIR/requirements.txt" ]]; then
        log_info "Installing cardbrick-cli dependencies..."
        "$VENV_DIR/bin/pip" install -r "$CLI_DIR/requirements.txt"
    fi
    
    log_success "Dependencies installed successfully"
}

# Create data directories
setup_data_directories() {
    log_info "Setting up data directories..."
    
    # Create main data directory
    if [[ ! -d "$DATA_DIR" ]]; then
        log_info "Creating data directory: $DATA_DIR"
        sudo mkdir -p "$DATA_DIR"
        sudo chown "$USER:$USER" "$DATA_DIR"
    fi
    
    # Create subdirectories
    mkdir -p "$DATA_DIR"/{keys,keys/devices,inbox,learners}
    
    log_success "Data directories created at $DATA_DIR"
}

# Generate SSH keys if they don't exist
setup_ssh_keys() {
    log_info "Setting up SSH keys..."
    
    DAEMON_KEY="$DATA_DIR/keys/daemon_private.pem"
    
    if [[ ! -f "$DAEMON_KEY" ]]; then
        log_info "Generating daemon SSH key..."
        ssh-keygen -t ed25519 -f "$DAEMON_KEY" -N "" -C "cardbrick-daemon@$(hostname)"
        chmod 600 "$DAEMON_KEY"
        chmod 644 "${DAEMON_KEY}.pub"
        log_success "Daemon SSH key generated"
    else
        log_info "Daemon SSH key already exists"
    fi
}

# Create startup script
create_startup_script() {
    log_info "Creating startup script..."
    
    cat > "$SCRIPT_DIR/start_daemon.sh" << EOF
#!/bin/bash
# CardBrick Sync Daemon Startup Script
# Generated by setup_daemon.sh

SCRIPT_DIR="\$(cd "\$(dirname "\${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="\$SCRIPT_DIR/venv-syncd"
DAEMON_DIR="\$SCRIPT_DIR/cardbrick-syncd"

# Check if virtual environment exists
if [[ ! -d "\$VENV_DIR" ]]; then
    echo "Virtual environment not found! Please run setup_daemon.sh first."
    exit 1
fi

# Set environment variables
export CARDBRICK_DATA_DIR="${DATA_DIR}"
export PYTHONPATH="\$DAEMON_DIR:\$SCRIPT_DIR/cardbrick-cli:\$PYTHONPATH"

# Activate virtual environment and run daemon
echo "Starting CardBrick Sync Daemon..."
echo "Data directory: \$CARDBRICK_DATA_DIR"
echo "HTTP port: \${CARDBRICK_HTTP_PORT:-6429}"
echo "Press Ctrl+C to stop"
echo

cd "\$DAEMON_DIR"
"\$VENV_DIR/bin/python" -m main "\$@"
EOF
    
    chmod +x "$SCRIPT_DIR/start_daemon.sh"
    log_success "Startup script created: $SCRIPT_DIR/start_daemon.sh"
}

# Create systemd service file
create_systemd_service() {
    log_info "Creating systemd service file..."
    
    cat > "$SCRIPT_DIR/cardbrick-syncd-venv.service" << EOF
[Unit]
Description=CardBrick Sync Daemon (Virtual Environment)
After=network.target
Wants=network.target

[Service]
Type=simple
User=$USER
Group=$USER
WorkingDirectory=$DAEMON_DIR
Environment=CARDBRICK_DATA_DIR=$DATA_DIR
Environment=PYTHONPATH=$DAEMON_DIR:$CLI_DIR
ExecStart=$VENV_DIR/bin/python -m main
Restart=always
RestartSec=10

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$DATA_DIR

[Install]
WantedBy=multi-user.target
EOF
    
    log_success "Systemd service file created: $SCRIPT_DIR/cardbrick-syncd-venv.service"
    log_info "To install as system service:"
    log_info "  sudo cp cardbrick-syncd-venv.service /etc/systemd/system/"
    log_info "  sudo systemctl daemon-reload"
    log_info "  sudo systemctl enable cardbrick-syncd-venv"
    log_info "  sudo systemctl start cardbrick-syncd-venv"
}

# Test the installation
test_installation() {
    log_info "Testing installation..."
    
    # Test that we can import the main modules
    if "$VENV_DIR/bin/python" -c "
import sys
sys.path.insert(0, '$DAEMON_DIR')
sys.path.insert(0, '$CLI_DIR')

try:
    from daemon import SyncDaemon
    from config import Config
    print('✓ Daemon modules import successfully')
except ImportError as e:
    print(f'✗ Import error: {e}')
    sys.exit(1)

try:
    from wormhole_backup import WormholeBackup
    print('✓ CLI modules import successfully')
except ImportError as e:
    print(f'✗ CLI import error: {e}')
    sys.exit(1)
"; then
        log_success "Installation test passed"
    else
        log_error "Installation test failed"
        return 1
    fi
}

# Main setup function
main() {
    echo "=================================="
    echo "CardBrick Sync Daemon Setup"
    echo "=================================="
    echo
    
    check_directory
    check_system_deps
    create_venv
    install_dependencies
    setup_data_directories
    setup_ssh_keys
    create_startup_script
    create_systemd_service
    
    echo
    log_info "Testing installation..."
    if test_installation; then
        echo
        echo "=================================="
        log_success "Setup completed successfully!"
        echo "=================================="
        echo
        echo "To start the daemon:"
        echo "  ./start_daemon.sh"
        echo
        echo "To run in background:"
        echo "  nohup ./start_daemon.sh > daemon.log 2>&1 &"
        echo
        echo "To install as system service:"
        echo "  sudo cp cardbrick-syncd-venv.service /etc/systemd/system/"
        echo "  sudo systemctl enable cardbrick-syncd-venv"
        echo "  sudo systemctl start cardbrick-syncd-venv"
        echo
        echo "Data directory: $DATA_DIR"
        echo "Virtual environment: $VENV_DIR"
        echo
    else
        log_error "Setup completed with errors. Please check the output above."
        exit 1
    fi
}

# Handle command line arguments
case "${1:-setup}" in
    "setup"|"install")
        main
        ;;
    "clean")
        log_info "Cleaning up virtual environment..."
        rm -rf "$VENV_DIR"
        rm -f "$SCRIPT_DIR/start_daemon.sh"
        rm -f "$SCRIPT_DIR/cardbrick-syncd-venv.service"
        log_success "Cleanup completed"
        ;;
    "test")
        if [[ -d "$VENV_DIR" ]]; then
            test_installation
        else
            log_error "Virtual environment not found. Run setup first."
            exit 1
        fi
        ;;
    *)
        echo "Usage: $0 [setup|clean|test]"
        echo "  setup (default): Create virtual environment and install dependencies"
        echo "  clean: Remove virtual environment and generated files"
        echo "  test: Test the installation"
        exit 1
        ;;
esac