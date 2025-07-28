#!/bin/bash
# clean_databases.sh - CardBrick Database Cleanup Utility
# 
# This script safely removes CardBrick databases and stored data.
# Use this to reset your study progress and start fresh.

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Data locations
USER_DATA_DIR="$HOME/.cardbrick"
DECK_HISTORY_DIR="$SCRIPT_DIR/anki/history"
CACHE_DIR="$SCRIPT_DIR/test_cache"
BUILD_DIR="$SCRIPT_DIR/target"

show_usage() {
    echo -e "${BLUE}CardBrick Database Cleanup Utility${NC}"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --help          Show this help message"
    echo "  --all           Clean all data without prompts"
    echo "  --backup        Create backup before cleaning"
    echo "  --user-data     Clean only user progress data (~/.cardbrick/)"
    echo "  --deck-history  Clean only deck learning history (./anki/history/)"
    echo "  --cache         Clean only cached deck data (./test_cache/)"
    echo "  --build         Clean only build artifacts (./target/)"
    echo ""
    echo "Interactive mode (default): Prompts for each category"
    echo ""
    echo -e "${YELLOW}Warning: This will permanently delete your study progress!${NC}"
}

create_backup() {
    local timestamp=$(date +"%Y%m%d_%H%M%S")
    local backup_dir="$SCRIPT_DIR/backup_$timestamp"
    
    echo -e "${BLUE}Creating backup at: $backup_dir${NC}"
    mkdir -p "$backup_dir"
    
    # Backup user data if exists
    if [ -d "$USER_DATA_DIR" ]; then
        echo "  - Backing up user progress data..."
        cp -r "$USER_DATA_DIR" "$backup_dir/cardbrick_userdata/"
    fi
    
    # Backup deck history if exists
    if [ -d "$DECK_HISTORY_DIR" ]; then
        echo "  - Backing up deck learning history..."
        cp -r "$DECK_HISTORY_DIR" "$backup_dir/anki_history/"
    fi
    
    # Backup cache if exists
    if [ -d "$CACHE_DIR" ]; then
        echo "  - Backing up cached deck data..."
        cp -r "$CACHE_DIR" "$backup_dir/test_cache/"
    fi
    
    echo -e "${GREEN}Backup created successfully!${NC}"
    echo ""
}

clean_user_data() {
    if [ -d "$USER_DATA_DIR" ]; then
        echo -e "${YELLOW}Removing user progress data: $USER_DATA_DIR${NC}"
        echo "  This includes: study progress, points, streaks, daily queues, profile data"
        rm -rf "$USER_DATA_DIR"
        echo -e "${GREEN}✓ User data cleaned${NC}"
    else
        echo "  No user data found to clean"
    fi
}

clean_deck_history() {
    if [ -d "$DECK_HISTORY_DIR" ]; then
        echo -e "${YELLOW}Removing deck learning history: $DECK_HISTORY_DIR${NC}"
        echo "  This includes: SRS states, card intervals, review logs, transaction logs"
        rm -rf "$DECK_HISTORY_DIR"
        echo -e "${GREEN}✓ Deck history cleaned${NC}"
    else
        echo "  No deck history found to clean"
    fi
}

clean_cache() {
    if [ -d "$CACHE_DIR" ]; then
        echo -e "${YELLOW}Removing cached deck data: $CACHE_DIR${NC}"
        echo "  This includes: pre-processed deck files, manifests, indexed databases"
        rm -rf "$CACHE_DIR"
        echo -e "${GREEN}✓ Cache cleaned${NC}"
        echo -e "${BLUE}Note: Run 'python precache_decks.py' to regenerate cache${NC}"
    else
        echo "  No cache found to clean"
    fi
}

clean_build() {
    if [ -d "$BUILD_DIR" ]; then
        echo -e "${YELLOW}Removing build artifacts: $BUILD_DIR${NC}"
        echo "  This includes: compilation cache, dependencies, target binaries"
        rm -rf "$BUILD_DIR"
        echo -e "${GREEN}✓ Build artifacts cleaned${NC}"
        echo -e "${BLUE}Note: Next 'cargo build' will take longer${NC}"
    else
        echo "  No build artifacts found to clean"
    fi
}

show_summary() {
    echo ""
    echo -e "${BLUE}=== Storage Locations Summary ===${NC}"
    echo -e "User Progress:    $USER_DATA_DIR $([ -d "$USER_DATA_DIR" ] && echo -e "${RED}[EXISTS]${NC}" || echo -e "${GREEN}[CLEAN]${NC}")"
    echo -e "Deck History:     $DECK_HISTORY_DIR $([ -d "$DECK_HISTORY_DIR" ] && echo -e "${RED}[EXISTS]${NC}" || echo -e "${GREEN}[CLEAN]${NC}")"
    echo -e "Cached Data:      $CACHE_DIR $([ -d "$CACHE_DIR" ] && echo -e "${RED}[EXISTS]${NC}" || echo -e "${GREEN}[CLEAN]${NC}")"
    echo -e "Build Artifacts:  $BUILD_DIR $([ -d "$BUILD_DIR" ] && echo -e "${RED}[EXISTS]${NC}" || echo -e "${GREEN}[CLEAN]${NC}")"
    echo ""
}

confirm_action() {
    local message="$1"
    echo -e "${YELLOW}$message${NC}"
    read -p "Continue? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        return 1
    fi
    return 0
}

interactive_cleanup() {
    echo -e "${BLUE}CardBrick Interactive Database Cleanup${NC}"
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
    
    if [ -d "$BUILD_DIR" ]; then
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

while [[ $# -gt 0 ]]; do
    case $1 in
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
    clean_build
    
    echo ""
    echo -e "${GREEN}Complete cleanup finished!${NC}"
    show_summary
    
elif [ "$CLEAN_USER" = true ] || [ "$CLEAN_HISTORY" = true ] || [ "$CLEAN_CACHE" = true ] || [ "$CLEAN_BUILD" = true ]; then
    # Specific cleanup mode
    echo -e "${BLUE}CardBrick Selective Database Cleanup${NC}"
    echo ""
    
    if [ "$BACKUP" = true ]; then
        create_backup
    fi
    
    [ "$CLEAN_USER" = true ] && clean_user_data
    [ "$CLEAN_HISTORY" = true ] && clean_deck_history
    [ "$CLEAN_CACHE" = true ] && clean_cache
    [ "$CLEAN_BUILD" = true ] && clean_build
    
    echo ""
    echo -e "${GREEN}Selective cleanup completed!${NC}"
    show_summary
    
else
    # Interactive mode (default)
    interactive_cleanup
fi