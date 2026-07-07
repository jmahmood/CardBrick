// src/test_db.rs
// Simple database connection tester for CardBrick

use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Card {
    pub id: i64,
    pub note_id: i64,
    pub due: i64,
    pub interval: u32,
    pub ease_factor: u32,
    pub lapses: u32,
}

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

#[derive(Clone)]
pub struct DeckMetadata {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
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

/// Load and parse a manifest file
pub fn load_manifest(manifest_path: &Path) -> Result<DeckManifest, String> {
    let content = fs::read_to_string(manifest_path)
        .map_err(|e| format!("Failed to read manifest file: {}", e))?;

    serde_json::from_str(&content).map_err(|e| format!("Failed to parse manifest JSON: {}", e))
}

/// Load cached decks from the cache directory
pub fn load_cached_decks() -> Result<Vec<DeckMetadata>, String> {
    let cache_root = find_cache_root();

    if !cache_root.exists() {
        return Ok(Vec::new());
    }

    let mut decks = Vec::new();
    let entries = fs::read_dir(&cache_root).map_err(|e| {
        format!(
            "Failed to read cache directory '{}': {}",
            cache_root.display(),
            e
        )
    })?;

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
                        eprintln!(
                            "Warning: Failed to load manifest at {}: {}",
                            manifest_path.display(),
                            e
                        );
                    }
                }
            }
        }
    }

    Ok(decks)
}

/// Ensure a cached deck exists and return the path to its database
pub fn ensure_cached_deck(deck_hash: &str) -> Result<PathBuf, String> {
    let cache_root = find_cache_root();
    let deck_dir = cache_root.join(deck_hash);

    if !deck_dir.exists() {
        return Err(format!(
            "Cached deck directory not found: {}",
            deck_dir.display()
        ));
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 CardBrick Database Connection Tester");
    println!("========================================");

    // Step 1: Find cache root and available decks
    let cache_root = find_cache_root();
    println!("📁 Cache root: {:?}", cache_root);

    if !cache_root.exists() {
        println!("❌ Cache root does not exist!");
        return Ok(());
    }

    // Step 2: Load cached decks
    println!("\n🔍 Scanning for cached decks...");
    let cached_decks = load_cached_decks()?;
    println!("📊 Found {} cached deck(s)", cached_decks.len());

    if cached_decks.is_empty() {
        println!("❌ No cached decks found!");
        return Ok(());
    }

    // Step 3: Test each cached deck
    for (i, deck_metadata) in cached_decks.iter().enumerate() {
        println!("\n{}", "=".repeat(50));
        println!("🎯 Testing Deck {}: {}", i + 1, deck_metadata.name);
        println!("   ID: {}", deck_metadata.id);
        println!("   Path: {:?}", deck_metadata.path);

        // Get database path
        let deck_hash = &deck_metadata.id;
        match ensure_cached_deck(deck_hash) {
            Ok(db_path) => {
                println!("✅ Database path: {:?}", db_path);
                test_database(&db_path)?;
            }
            Err(e) => {
                println!("❌ Failed to find database: {}", e);
            }
        }
    }

    // Step 4: Test the specific JLPT N1 deck that should have 2699 cards
    println!("\n{}", "=".repeat(50));
    println!("🎯 Testing JLPT N1 Vocab Deck Specifically");
    let jlpt_n1_hash = "7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6";

    match ensure_cached_deck(jlpt_n1_hash) {
        Ok(db_path) => {
            println!("✅ JLPT N1 Database path: {:?}", db_path);
            test_load_more_cards_logic(&db_path)?;
        }
        Err(e) => {
            println!("❌ Failed to find JLPT N1 database: {}", e);
        }
    }

    Ok(())
}

fn test_database(db_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("  🔌 Opening database connection...");

    // Open with the same flags as CardBrick
    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;

    // Apply the same PRAGMAs as CardBrick
    conn.pragma_update(None, "journal_mode", "OFF")?;
    conn.pragma_update(None, "synchronous", "OFF")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    println!("  ✅ Database opened with CardBrick settings");

    // Test 1: Count total cards
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM cards")?;
    let total_cards: i64 = stmt.query_row([], |row| row.get(0))?;
    println!("  📊 Total cards in database: {}", total_cards);

    // Test 2: Count total notes
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM notes")?;
    let total_notes: i64 = stmt.query_row([], |row| row.get(0))?;
    println!("  📝 Total notes in database: {}", total_notes);

    // Test 3: Get sample card data
    println!("  🔍 Sample card data:");
    let mut stmt = conn.prepare("SELECT id, nid, due, ivl, factor, lapses FROM cards LIMIT 5")?;
    let card_rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?, // id
            row.get::<_, i64>(1)?, // nid (note_id)
            row.get::<_, i64>(2)?, // due
            row.get::<_, u32>(3)?, // ivl (interval)
            row.get::<_, u32>(4)?, // factor
            row.get::<_, u32>(5)?, // lapses
        ))
    })?;

    for (i, card_result) in card_rows.enumerate() {
        match card_result {
            Ok((id, nid, due, ivl, factor, lapses)) => {
                println!(
                    "    Card {}: id={}, note_id={}, due={}, interval={}, factor={}, lapses={}",
                    i + 1,
                    id,
                    nid,
                    due,
                    ivl,
                    factor,
                    lapses
                );
            }
            Err(e) => {
                println!("    ❌ Error reading card {}: {}", i + 1, e);
            }
        }
    }

    // Test 4: Check current timestamp logic (same as CardBrick)
    let today = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86400; // Convert to days
    println!("  📅 Today (Anki format): {}", today);

    // Test 5: Test due cards query (same as CardBrick loader)
    let mut stmt = conn.prepare(
        "SELECT COUNT(*) FROM cards 
         WHERE due <= ? OR ivl = 0",
    )?;
    let due_cards: i64 = stmt.query_row([today as i64], |row| row.get(0))?;
    println!("  ⏰ Cards due today or new: {}", due_cards);

    // Test 6: Test random card selection
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM cards ORDER BY RANDOM() LIMIT 100")?;
    let random_cards: i64 = stmt.query_row([], |row| row.get(0))?;
    println!(
        "  🎲 Random card query test (should be <= 100): {}",
        random_cards
    );

    println!("  ✅ Database tests completed successfully");
    Ok(())
}

fn test_load_more_cards_logic(db_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("  🧪 Testing load_more_cards logic simulation...");

    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;

    // Apply CardBrick PRAGMAs
    conn.pragma_update(None, "journal_mode", "OFF")?;
    conn.pragma_update(None, "synchronous", "OFF")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;

    let limit = 10; // Same as CardBrick's minimum
    let failed_today: Vec<i64> = vec![];
    let hard_today: Vec<i64> = vec![];
    let _existing_ids: HashSet<i64> = HashSet::new(); // Simulating empty existing cards

    println!(
        "  📊 Simulating load_more_cards with limit={}, failed_today={}, hard_today={}",
        limit,
        failed_today.len(),
        hard_today.len()
    );

    // Strategy 1: Cards from failed/hard today (should be empty for this test)
    let _difficult_cards = if failed_today.is_empty() && hard_today.is_empty() {
        0
    } else {
        // This would normally run the difficult cards query
        println!("    (Skipping difficult cards - none provided for test)");
        0
    };

    // Strategy 2: Uncovered cards (cards with no srs_log entry)
    println!("  🔍 Strategy 2: Testing uncovered cards query...");
    let uncovered_query = "SELECT c.id, c.nid, c.due, c.ivl, c.factor, c.lapses 
         FROM cards c 
         LEFT JOIN srs_log s ON c.id = s.card_id 
         WHERE s.card_id IS NULL 
         ORDER BY RANDOM() 
         LIMIT ?";

    let mut stmt = conn.prepare(uncovered_query)?;
    let uncovered_cards: Vec<_> = stmt
        .query_map([limit as i64], |row| {
            Ok(Card {
                id: row.get(0)?,
                note_id: row.get(1)?,
                due: row.get(2)?,
                interval: row.get(3)?,
                ease_factor: row.get(4)?,
                lapses: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    println!("  📈 Uncovered cards found: {}", uncovered_cards.len());

    // Strategy 3: Random cards if still need more
    let cards_still_needed = if uncovered_cards.len() < limit {
        limit - uncovered_cards.len()
    } else {
        0
    };

    if cards_still_needed > 0 {
        println!(
            "  🎲 Strategy 3: Testing random cards query (need {} more)...",
            cards_still_needed
        );

        let random_query = "SELECT id, nid, due, ivl, factor, lapses 
             FROM cards 
             ORDER BY RANDOM() 
             LIMIT ?";

        let mut stmt = conn.prepare(random_query)?;
        let random_cards: Vec<_> = stmt
            .query_map([cards_still_needed as i64], |row| {
                Ok(Card {
                    id: row.get(0)?,
                    note_id: row.get(1)?,
                    due: row.get(2)?,
                    interval: row.get(3)?,
                    ease_factor: row.get(4)?,
                    lapses: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        println!("  📈 Additional random cards found: {}", random_cards.len());
        println!(
            "  ✅ Total cards would be loaded: {}",
            uncovered_cards.len() + random_cards.len()
        );
    } else {
        println!(
            "  ✅ Total cards would be loaded: {} (no additional cards needed)",
            uncovered_cards.len()
        );
    }

    // Additional diagnostic: Check if srs_log table exists and has data
    println!("  🔍 Checking srs_log table...");
    match conn.prepare("SELECT COUNT(*) FROM srs_log") {
        Ok(mut stmt) => match stmt.query_row([], |row| row.get::<_, i64>(0)) {
            Ok(count) => println!("  📊 srs_log entries: {}", count),
            Err(e) => println!("  ⚠️  Error querying srs_log: {}", e),
        },
        Err(e) => {
            println!("  ⚠️  srs_log table may not exist: {}", e);
            println!("  💡 This means ALL cards should be considered 'uncovered'");
        }
    }

    // Test the original CardBrick due cards logic too
    println!("  🔍 Testing original due cards logic...");
    let today = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86400;

    let due_query = "SELECT id, nid, due, ivl, factor, lapses FROM cards 
         WHERE due <= ? OR ivl = 0 
         ORDER BY due ASC, ivl ASC 
         LIMIT 100";

    let mut stmt = conn.prepare(due_query)?;
    let due_cards: Vec<_> = stmt
        .query_map([today as i64], |row| {
            Ok(Card {
                id: row.get(0)?,
                note_id: row.get(1)?,
                due: row.get(2)?,
                interval: row.get(3)?,
                ease_factor: row.get(4)?,
                lapses: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    println!("  ⏰ Due cards found: {}", due_cards.len());

    println!("  ✅ load_more_cards simulation completed");
    Ok(())
}
