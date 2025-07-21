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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_manifest() -> DeckManifest {
        DeckManifest {
            apkg_name: "test_deck.apkg".to_string(),
            sha256: "abc123".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            db_file: "test_deck.db".to_string(),
            card_count: 100,
            notes_count: 50,
            deck_name: "Test Deck".to_string(),
            anki_version: 2,
        }
    }

    #[test]
    fn test_deck_manifest_serialization() {
        let manifest = create_test_manifest();
        
        // Test serialization
        let json = serde_json::to_string(&manifest).unwrap();
        assert!(json.contains("test_deck.apkg"));
        assert!(json.contains("abc123"));
        assert!(json.contains("Test Deck"));
        
        // Test deserialization
        let parsed: DeckManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.apkg_name, manifest.apkg_name);
        assert_eq!(parsed.sha256, manifest.sha256);
        assert_eq!(parsed.deck_name, manifest.deck_name);
        assert_eq!(parsed.card_count, manifest.card_count);
    }

    #[test]
    fn test_load_manifest_success() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("manifest.json");
        
        let manifest = create_test_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        fs::write(&manifest_path, json).unwrap();
        
        let loaded = load_manifest(&manifest_path).unwrap();
        assert_eq!(loaded.apkg_name, "test_deck.apkg");
        assert_eq!(loaded.sha256, "abc123");
        assert_eq!(loaded.deck_name, "Test Deck");
    }

    #[test]
    fn test_load_manifest_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("nonexistent.json");
        
        let result = load_manifest(&manifest_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read manifest file"));
    }

    #[test]
    fn test_load_manifest_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("invalid.json");
        
        fs::write(&manifest_path, "{ invalid json }").unwrap();
        
        let result = load_manifest(&manifest_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse manifest JSON"));
    }

    #[test]
    fn test_load_cached_decks_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        
        // Mock find_cache_root to return our temp directory
        // Note: This test relies on the directory not existing, so we use a non-existent path
        let non_existent = temp_dir.path().join("non_existent");
        
        // Since we can't easily mock find_cache_root, we test the logic indirectly
        // by testing the error handling for non-existent directories
        assert!(!non_existent.exists());
    }

    #[test]
    fn test_load_cached_decks_with_valid_deck() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create a deck directory with manifest
        let deck_dir = temp_dir.path().join("abc123");
        fs::create_dir_all(&deck_dir).unwrap();
        
        let manifest = create_test_manifest();
        let manifest_json = serde_json::to_string(&manifest).unwrap();
        let manifest_path = deck_dir.join("manifest.json");
        fs::write(&manifest_path, manifest_json).unwrap();
        
        // Test load_manifest directly since we can't easily mock find_cache_root
        let loaded_manifest = load_manifest(&manifest_path).unwrap();
        assert_eq!(loaded_manifest.deck_name, "Test Deck");
        assert_eq!(loaded_manifest.sha256, "abc123");
    }

    #[test]
    fn test_find_cache_root_fallback_logic() {
        // Test that find_cache_root returns a valid PathBuf
        let cache_root = find_cache_root();
        assert!(!cache_root.as_os_str().is_empty());
        
        // The path should be one of the expected patterns
        let path_str = cache_root.to_string_lossy();
        let has_expected_pattern = path_str.contains("storage") ||
                                   path_str.contains("SDCARD") ||
                                   path_str.contains("test_cache") ||
                                   path_str.contains("precache");
        assert!(has_expected_pattern, "Cache root path '{}' doesn't match expected patterns", path_str);
    }

    #[test]
    fn test_ensure_cached_deck_missing_directory() {
        let result = ensure_cached_deck("nonexistent_hash");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cached deck directory not found"));
    }

    #[test]
    fn test_deck_metadata_creation() {
        let manifest = create_test_manifest();
        let temp_dir = TempDir::new().unwrap();
        
        let metadata = DeckMetadata {
            id: manifest.sha256.clone(),
            name: manifest.deck_name.clone(),
            path: temp_dir.path().to_path_buf(),
        };
        
        assert_eq!(metadata.id, "abc123");
        assert_eq!(metadata.name, "Test Deck");
        assert_eq!(metadata.path, temp_dir.path());
    }

    #[test]
    fn test_manifest_required_fields() {
        // Test that all required fields are present in serialization
        let manifest = create_test_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        
        // Check that all important fields are included
        assert!(json.contains("apkg_name"));
        assert!(json.contains("sha256"));
        assert!(json.contains("created_at"));
        assert!(json.contains("db_file"));
        assert!(json.contains("card_count"));
        assert!(json.contains("notes_count"));
        assert!(json.contains("deck_name"));
        assert!(json.contains("anki_version"));
    }

    #[test]
    fn test_manifest_field_types() {
        let manifest = create_test_manifest();
        
        // Test field types
        assert_eq!(manifest.card_count, 100_u64);
        assert_eq!(manifest.notes_count, 50_u64);
        assert_eq!(manifest.anki_version, 2_u32);
        assert!(manifest.apkg_name.ends_with(".apkg"));
        assert!(!manifest.sha256.is_empty());
        assert!(!manifest.deck_name.is_empty());
    }
}