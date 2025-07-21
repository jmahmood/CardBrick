#!/usr/bin/env python3
"""
CardBrick Deck Pre-cache Utility

This script pre-processes .apkg files into a local cache directory for faster loading.
It extracts the SQLite database, adds performance indexes, and creates metadata manifests.

Usage:
    python precache_decks.py SOURCE_DIR CACHE_ROOT

Example:
    python precache_decks.py ~/Downloads/decks /storage/cardbrick/decks
"""

import argparse
import hashlib
import json
import sqlite3
import sys
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional


def calculate_sha256(file_path: Path) -> str:
    """Calculate SHA-256 hash of a file."""
    sha256_hash = hashlib.sha256()
    with open(file_path, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            sha256_hash.update(chunk)
    return sha256_hash.hexdigest()


def find_apkg_files(source_dir: Path) -> List[Path]:
    """Find all .apkg files in the source directory."""
    if not source_dir.exists():
        raise FileNotFoundError(f"Source directory does not exist: {source_dir}")
    
    return list(source_dir.glob("*.apkg"))


def extract_deck_info(db_path: Path) -> Dict:
    """Extract metadata from the Anki database."""
    conn = sqlite3.connect(db_path, isolation_level=None)
    try:
        # Get basic counts
        cursor = conn.execute("SELECT COUNT(*) FROM cards")
        card_count = cursor.fetchone()[0]
        
        cursor = conn.execute("SELECT COUNT(*) FROM notes")
        notes_count = cursor.fetchone()[0]
        
        # Try to get deck name from col table
        deck_name = "Unknown Deck"
        try:
            cursor = conn.execute("SELECT decks FROM col LIMIT 1")
            decks_json = cursor.fetchone()
            if decks_json:
                decks_data = json.loads(decks_json[0])
                # Get the first deck name (skip default deck with ID "1")
                for deck_id, deck_info in decks_data.items():
                    if deck_id != "1" and isinstance(deck_info, dict) and "name" in deck_info:
                        deck_name = deck_info["name"]
                        break
        except (sqlite3.Error, json.JSONDecodeError, KeyError):
            pass
        
        # Determine Anki version based on schema
        anki_version = 21  # Default
        try:
            cursor = conn.execute("PRAGMA table_info(col)")
            columns = [row[1] for row in cursor.fetchall()]
            if "ver" in columns:
                anki_version = 21
            else:
                anki_version = 2
        except sqlite3.Error:
            pass
        
        return {
            "card_count": card_count,
            "notes_count": notes_count,
            "deck_name": deck_name,
            "anki_version": anki_version
        }
    finally:
        conn.close()


def add_performance_indexes(db_path: Path) -> None:
    """Add performance indexes to the database."""
    conn = sqlite3.connect(db_path, isolation_level=None)
    try:
        # Add indexes for card queries used by CardBrick
        conn.execute("CREATE INDEX IF NOT EXISTS idx_cards_due ON cards(due)")
        conn.execute("CREATE INDEX IF NOT EXISTS idx_cards_ivl ON cards(ivl)")
        
        # Optimize database
        conn.execute("VACUUM")
        print(f"Added performance indexes to {db_path.name}")
    finally:
        conn.close()


def process_apkg(apkg_path: Path, cache_root: Path) -> bool:
    """Process a single .apkg file into the cache."""
    print(f"Processing: {apkg_path.name}")
    
    # Calculate hash
    file_hash = calculate_sha256(apkg_path)
    cache_dir = cache_root / file_hash
    
    # Skip if already cached
    if cache_dir.exists() and (cache_dir / "manifest.json").exists():
        print(f"  Already cached, skipping")
        return True
    
    # Create cache directory
    cache_dir.mkdir(parents=True, exist_ok=True)
    
    try:
        # Extract ZIP archive
        with zipfile.ZipFile(apkg_path, 'r') as archive:
            # Determine database filename
            db_filename = None
            if "collection.anki21" in archive.namelist():
                db_filename = "collection.anki21"
            elif "collection.anki2" in archive.namelist():
                db_filename = "collection.anki2"
            else:
                raise ValueError("No Anki database found in archive")
            
            # Extract database
            db_path = cache_dir / db_filename
            with archive.open(db_filename) as db_file:
                with open(db_path, 'wb') as out_file:
                    out_file.write(db_file.read())
            
            # Extract media directory if present
            media_files = [name for name in archive.namelist() if not name.endswith(('.anki2', '.anki21'))]
            if media_files:
                media_dir = cache_dir / "media"
                media_dir.mkdir(exist_ok=True)
                for media_file in media_files:
                    if not media_file.endswith('/'):  # Skip directories
                        archive.extract(media_file, media_dir)
        
        # Add performance indexes
        add_performance_indexes(db_path)
        
        # Extract deck metadata
        deck_info = extract_deck_info(db_path)
        
        # Create manifest
        manifest = {
            "apkg_name": apkg_path.name,
            "sha256": file_hash,
            "created_at": datetime.now(timezone.utc).isoformat(),
            "db_file": db_filename,
            "card_count": deck_info["card_count"],
            "notes_count": deck_info["notes_count"],
            "deck_name": deck_info["deck_name"],
            "anki_version": deck_info["anki_version"]
        }
        
        # Write manifest
        manifest_path = cache_dir / "manifest.json"
        with open(manifest_path, 'w', encoding='utf-8') as f:
            json.dump(manifest, f, indent=2, ensure_ascii=False)
        
        print(f"  Cached successfully: {deck_info['card_count']} cards, {deck_info['notes_count']} notes")
        return True
        
    except Exception as e:
        print(f"  Error processing {apkg_path.name}: {e}")
        # Clean up partial cache on error
        if cache_dir.exists():
            import shutil
            shutil.rmtree(cache_dir)
        return False


def main():
    parser = argparse.ArgumentParser(
        description="Pre-cache Anki .apkg files for CardBrick",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument("source_dir", type=Path, help="Directory containing .apkg files")
    parser.add_argument("cache_root", type=Path, help="Cache root directory")
    
    args = parser.parse_args()
    
    try:
        # Find .apkg files
        apkg_files = find_apkg_files(args.source_dir)
        if not apkg_files:
            print(f"No .apkg files found in {args.source_dir}")
            return 0
        
        print(f"Found {len(apkg_files)} .apkg files to process")
        
        # Ensure cache root exists
        args.cache_root.mkdir(parents=True, exist_ok=True)
        
        # Process each file
        successful = 0
        for apkg_file in apkg_files:
            if process_apkg(apkg_file, args.cache_root):
                successful += 1
        
        print(f"\nCompleted: {successful}/{len(apkg_files)} decks cached successfully")
        return 0 if successful == len(apkg_files) else 1
        
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())