// src/deck/scanner.rs
// This module handles discovery of cached decks

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::state::DeckMetadata;

/// Manifest structure for cached decks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckManifest {
    pub apkg_name: String,
    pub sha256: String,
    pub created_at: String,
    pub db_file: String,
    pub card_count: u64,
    pub notes_count: u64,
    pub deck_name: String,
    pub anki_version: u32,
}

/// Find the cache root directory based on the platform
pub fn find_cache_root() -> PathBuf {
    if Path::new("/storage").exists() {
        // RG35XX Plus - use storage partition
        PathBuf::from("/storage/applications/CardBrick/decks")
    } else if Path::new("/mnt/SDCARD").exists() {
        // TrimUI Brick - use SD card
        PathBuf::from("/mnt/SDCARD/cardbrick/decks")
    } else {
        // Desktop/development - use test_cache if it exists, otherwise precache
        let test_cache = PathBuf::from("./test_cache");
        if test_cache.exists() {
            test_cache
        } else {
            PathBuf::from("./precache")
        }
    }
}

/// Load cached decks from the cache directory
pub fn load_cached_decks() -> Result<Vec<DeckMetadata>, String> {
    let cache_root = find_cache_root();
    
    if !cache_root.exists() {
        return Ok(Vec::new());
    }
    
    let mut decks = Vec::new();
    let entries = fs::read_dir(&cache_root)
        .map_err(|e| format!("Failed to read cache directory '{}': {}", cache_root.display(), e))?;
    
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read cache directory entry: {}", e))?;
        let path = entry.path();
        
        if path.is_dir() {
            let manifest_path = path.join("manifest.json");
            if manifest_path.exists() {
                match load_manifest(&manifest_path) {
                    Ok(manifest) => {
                        let deck_id = manifest.sha256.clone();
                        decks.push(DeckMetadata {
                            id: deck_id,
                            name: manifest.deck_name,
                            path: path.clone(),
                        });
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load manifest at {}: {}", manifest_path.display(), e);
                    }
                }
            }
        }
    }
    
    Ok(decks)
}

/// Load and parse a manifest file
pub fn load_manifest(manifest_path: &Path) -> Result<DeckManifest, String> {
    let content = fs::read_to_string(manifest_path)
        .map_err(|e| format!("Failed to read manifest file: {}", e))?;
    
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse manifest JSON: {}", e))
}

/// Ensure a cached deck exists and return the path to its database
pub fn ensure_cached_deck(deck_hash: &str) -> Result<PathBuf, String> {
    let cache_root = find_cache_root();
    let deck_dir = cache_root.join(deck_hash);
    
    if !deck_dir.exists() {
        return Err(format!("Cached deck directory not found: {}", deck_dir.display()));
    }
    
    let manifest_path = deck_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(format!("Manifest not found: {}", manifest_path.display()));
    }
    
    let manifest = load_manifest(&manifest_path)?;
    let db_path = deck_dir.join(&manifest.db_file);
    
    if !db_path.exists() {
        return Err(format!("Database file not found: {}", db_path.display()));
    }
    
    Ok(db_path)
}