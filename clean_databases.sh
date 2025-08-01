#!/bin/bash
# clean_databases_universal.sh - CardBrick Universal Database Cleanup Utility
# 
# Compatible with Desktop, TrimUI Brick, and RG35XX Plus devices
# This script safely removes CardBrick databases and stored data.

set -e  # Exit on any error

# Device detection (same logic as CardBrick main app)
detect_device() {
    if [ -d "/mnt/SDCARD/Tools" ]; then
        echo "TrimUIBrick"
    elif [ -f "/storage/.config/distribution" ]; then
        echo "RG35XXPlus"
    else
        echo "Desktop"
    fi
}

# Initialize device-specific settings
DEVICE_TYPE=$(detect_device)

# Colors for output (only use on desktop or if terminal supports it)
if [ "$DEVICE_TYPE" = "Desktop" ] && [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    # No colors on handheld devices for compatibility
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Device-specific paths
get_user_data_dir() {
    case "$DEVICE_TYPE" in
        "TrimUIBrick")
            echo "/mnt/SDCARD/.cardbrick"
            ;;
        "RG35XXPlus")
            echo "/storage/.cardbrick"
            ;;
        "Desktop")
            echo "$HOME/.cardbrick"
            ;;
        *)
            echo "$HOME/.cardbrick"
            ;;
    esac
}

get_deck_history_dir() {
    case "$DEVICE_TYPE" in
        "TrimUIBrick")
            echo "/mnt/SDCARD/cardbrick/anki/history"
            ;;
        "RG35XXPlus")
            echo "/storage/applications/CardBrick/anki/history"
            ;;
        "Desktop")
            echo "$SCRIPT_DIR/anki/history"
            ;;
        *)
            echo "$SCRIPT_DIR/anki/history"
            ;;
    esac
}

get_cache_dir() {
    case "$DEVICE_TYPE" in
        "TrimUIBrick")
            echo "/mnt/SDCARD/cardbrick/test_cache"
            ;;
        "RG35XXPlus")
            echo "/storage/applications/CardBrick/test_cache"
            ;;
        "Desktop")
            echo "$SCRIPT_DIR/test_cache"
            ;;
        *)
            echo "$SCRIPT_DIR/test_cache"
            ;;
    esac
}

get_build_dir() {
    # Build directory only relevant on desktop
    case "$DEVICE_TYPE" in
        "Desktop")
            echo "$SCRIPT_DIR/target"
            ;;
        *)
            echo ""  # No build directory on devices
            ;;
    esac
}

# Data locations
USER_DATA_DIR=$(get_user_data_dir)
DECK_HISTORY_DIR=$(get_deck_history_dir)
CACHE_DIR=$(get_cache_dir)
BUILD_DIR=$(get_build_dir)

show_usage() {
    echo -e "${BLUE}CardBrick Universal Database Cleanup Utility${NC}"
    echo "Device: $DEVICE_TYPE"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --help          Show this help message"
    echo "  --all           Clean all data without prompts"
    echo "  --backup        Create backup before cleaning"
    echo "  --user-data     Clean only user progress data"
    echo "  --deck-history  Clean only deck learning history"
    echo "  --cache         Clean only cached deck data"
    if [ -n "$BUILD_DIR" ]; then
        echo "  --build         Clean only build artifacts"
    fi
    echo ""
    echo "Interactive mode (default): Prompts for each category"
    echo ""
    echo -e "${YELLOW}Warning: This will permanently delete your study progress!${NC}"
}

create_backup() {
    local timestamp=$(date +"%Y%m%d_%H%M%S" 2>/dev/null || date +"%Y%m%d")
    local backup_base_dir
    
    # Choose backup location based on device
    case "$DEVICE_TYPE" in
        "TrimUIBrick")
            backup_base_dir="/mnt/SDCARD/cardbrick_backup"
            ;;
        "RG35XXPlus")
            backup_base_dir="/storage/cardbrick_backup"
            ;;
        *)
            backup_base_dir="$SCRIPT_DIR/backup"
            ;;
    esac
    
    local backup_dir="$backup_base_dir/backup_$timestamp"
    
    echo -e "${BLUE}Creating backup at: $backup_dir${NC}"
    mkdir -p "$backup_dir" || {
        echo -e "${RED}Failed to create backup directory${NC}"
        return 1
    }
    
    # Backup user data if exists
    if [ -d "$USER_DATA_DIR" ]; then
        echo "  - Backing up user progress data..."
        cp -r "$USER_DATA_DIR" "$backup_dir/cardbrick_userdata/" 2>/dev/null || echo "    Warning: Some files could not be backed up"
    fi
    
    # Backup deck history if exists
    if [ -d "$DECK_HISTORY_DIR" ]; then
        echo "  - Backing up deck learning history..."
        cp -r "$DECK_HISTORY_DIR" "$backup_dir/anki_history/" 2>/dev/null || echo "    Warning: Some files could not be backed up"
    fi
    
    # Backup cache if exists
    if [ -d "$CACHE_DIR" ]; then
        echo "  - Backing up cached deck data..."
        cp -r "$CACHE_DIR" "$backup_dir/test_cache/" 2>/dev/null || echo "    Warning: Some files could not be backed up"
    fi
    
    echo -e "${GREEN}Backup created successfully!${NC}"
    echo ""
}

clean_user_data() {
    if [ -d "$USER_DATA_DIR" ]; then
        echo -e "${YELLOW}Removing user progress data: $USER_DATA_DIR${NC}"
        echo "  This includes: study progress, points, streaks, daily queues, profile data"
        rm -rf "$USER_DATA_DIR" && echo -e "${GREEN}✓ User data cleaned${NC}" || echo -e "${RED}✗ Failed to clean user data${NC}"
    else
        echo "  No user data found to clean"
    fi
}

clean_deck_history() {
    if [ -d "$DECK_HISTORY_DIR" ]; then
        echo -e "${YELLOW}Removing deck learning history: $DECK_HISTORY_DIR${NC}"
        echo "  This includes: SRS states, card intervals, review logs, transaction logs"
        rm -rf "$DECK_HISTORY_DIR" && echo -e "${GREEN}✓ Deck history cleaned${NC}" || echo -e "${RED}✗ Failed to clean deck history${NC}"
    else
        echo "  No deck history found to clean"
    fi
}

clean_cache() {
    if [ -d "$CACHE_DIR" ]; then
        echo -e "${YELLOW}Removing cached deck data: $CACHE_DIR${NC}"
        echo "  This includes: pre-processed deck files, manifests, indexed databases"
        rm -rf "$CACHE_DIR" && echo -e "${GREEN}✓ Cache cleaned${NC}" || echo -e "${RED}✗ Failed to clean cache${NC}"
        echo -e "${BLUE}Note: Run 'python precache_decks.py' to regenerate cache${NC}"
    else
        echo "  No cache found to clean"
    fi
}

clean_build() {
    if [ -n "$BUILD_DIR" ] && [ -d "$BUILD_DIR" ]; then
        echo -e "${YELLOW}Removing build artifacts: $BUILD_DIR${NC}"
        echo "  This includes: compilation cache, dependencies, target binaries"
        rm -rf "$BUILD_DIR" && echo -e "${GREEN}✓ Build artifacts cleaned${NC}" || echo -e "${RED}✗ Failed to clean build artifacts${NC}"
        echo -e "${BLUE}Note: Next 'cargo build' will take longer${NC}"
    else
        echo "  No build artifacts found to clean (not applicable on this device)"
    fi
}

show_summary() {
    echo ""
    echo -e "${BLUE}=== Storage Locations Summary ($DEVICE_TYPE) ===${NC}"
    echo -e "User Progress:    $USER_DATA_DIR $([ -d "$USER_DATA_DIR" ] && echo -e "${RED}[EXISTS]${NC}" || echo -e "${GREEN}[CLEAN]${NC}")"
    echo -e "Deck History:     $DECK_HISTORY_DIR $([ -d "$DECK_HISTORY_DIR" ] && echo -e "${RED}[EXISTS]${NC}" || echo -e "${GREEN}[CLEAN]${NC}")"
    echo -e "Cached Data:      $CACHE_DIR $([ -d "$CACHE_DIR" ] && echo -e "${RED}[EXISTS]${NC}" || echo -e "${GREEN}[CLEAN]${NC}")"
    if [ -n "$BUILD_DIR" ]; then
        echo -e "Build Artifacts:  $BUILD_DIR $([ -d "$BUILD_DIR" ] && echo -e "${RED}[EXISTS]${NC}" || echo -e "${GREEN}[CLEAN]${NC}")"
    fi
    echo ""
}

# Simple confirmation function compatible with minimal bash
confirm_action() {
    local message="$1"
    echo -e "${YELLOW}$message${NC}"
    printf "Continue? (y/N): "
    read -r reply
    case "$reply" in
        [Yy]|[Yy][Ee][Ss])
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

interactive_cleanup() {
    echo -e "${BLUE}CardBrick Universal Database Cleanup${NC}"
    echo "Device: $DEVICE_TYPE"
    echo ""
    
    show_summary
    
    echo -e "${RED}WARNING: This will permanently delete your study data!${NC}"
    echo ""
    
    local create_backup=false
    if confirm_action "Create backup before cleaning?"; then
        create_backup=true
    fi
    echo ""
    
    # Ask for each category
    local clean_user=false
    local clean_history=false
    local clean_cache_data=false
    local clean_build_data=false
    
    if [ -d "$USER_DATA_DIR" ]; then
        if confirm_action "Clean user progress data? (points, streaks, study sessions)"; then
            clean_user=true
        fi
    fi
    
    if [ -d "$DECK_HISTORY_DIR" ]; then
        if confirm_action "Clean deck learning history? (card intervals, SRS states)"; then
            clean_history=true
        fi
    fi
    
    if [ -d "$CACHE_DIR" ]; then
        if confirm_action "Clean cached deck data? (will need to regenerate)"; then
            clean_cache_data=true
        fi
    fi
    
    if [ -n "$BUILD_DIR" ] && [ -d "$BUILD_DIR" ]; then
        if confirm_action "Clean build artifacts? (will slow next build)"; then
            clean_build_data=true
        fi
    fi
    
    # Check if anything to do
    if [ "$clean_user" = false ] && [ "$clean_history" = false ] && [ "$clean_cache_data" = false ] && [ "$clean_build_data" = false ]; then
        echo "No cleanup operations selected. Exiting."
        exit 0
    fi
    
    # Final confirmation
    echo ""
    if ! confirm_action "Proceed with cleanup? This action cannot be undone!"; then
        echo "Cleanup cancelled."
        exit 0
    fi
    
    echo ""
    echo -e "${BLUE}Starting cleanup...${NC}"
    
    # Create backup if requested
    if [ "$create_backup" = true ]; then
        create_backup
    fi
    
    # Perform cleanup
    [ "$clean_user" = true ] && clean_user_data
    [ "$clean_history" = true ] && clean_deck_history  
    [ "$clean_cache_data" = true ] && clean_cache
    [ "$clean_build_data" = true ] && clean_build
    
    echo ""
    echo -e "${GREEN}Cleanup completed successfully!${NC}"
    
    show_summary
}

# Parse command line arguments
BACKUP=false
CLEAN_ALL=false
CLEAN_USER=false
CLEAN_HISTORY=false
CLEAN_CACHE=false
CLEAN_BUILD=false

while [ $# -gt 0 ]; do
    case "$1" in
        --help)
            show_usage
            exit 0
            ;;
        --all)
            CLEAN_ALL=true
            shift
            ;;
        --backup)
            BACKUP=true
            shift
            ;;
        --user-data)
            CLEAN_USER=true
            shift
            ;;
        --deck-history)
            CLEAN_HISTORY=true
            shift
            ;;
        --cache)
            CLEAN_CACHE=true
            shift
            ;;
        --build)
            CLEAN_BUILD=true
            shift
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            show_usage
            exit 1
            ;;
    esac
done

# Main execution
if [ "$CLEAN_ALL" = true ]; then
    # Clean everything mode
    echo -e "${BLUE}CardBrick Complete Database Cleanup${NC}"
    echo "Device: $DEVICE_TYPE"
    echo ""
    show_summary
    echo -e "${RED}WARNING: This will delete ALL CardBrick data!${NC}"
    echo ""
    
    if [ "$BACKUP" = true ]; then
        create_backup
    fi
    
    if ! confirm_action "Delete ALL CardBrick databases and progress?"; then
        echo "Cleanup cancelled."
        exit 0
    fi
    
    echo ""
    echo -e "${BLUE}Cleaning all data...${NC}"
    clean_user_data
    clean_deck_history
    clean_cache
    [ -n "$BUILD_DIR" ] && clean_build
    
    echo ""
    echo -e "${GREEN}Complete cleanup finished!${NC}"
    show_summary
    
elif [ "$CLEAN_USER" = true ] || [ "$CLEAN_HISTORY" = true ] || [ "$CLEAN_CACHE" = true ] || [ "$CLEAN_BUILD" = true ]; then
    # Specific cleanup mode
    echo -e "${BLUE}CardBrick Selective Database Cleanup${NC}"
    echo "Device: $DEVICE_TYPE"
    echo ""
    
    if [ "$BACKUP" = true ]; then
        create_backup
    fi
    
    [ "$CLEAN_USER" = true ] && clean_user_data
    [ "$CLEAN_HISTORY" = true ] && clean_deck_history
    [ "$CLEAN_CACHE" = true ] && clean_cache
    [ "$CLEAN_BUILD" = true ] && [ -n "$BUILD_DIR" ] && clean_build
    
    echo ""
    echo -e "${GREEN}Selective cleanup completed!${NC}"
    show_summary
    
else
    # Interactive mode (default)
    interactive_cleanup
fi