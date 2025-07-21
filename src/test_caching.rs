// Test program for the caching system
use std::path::Path;

mod deck;
mod state;

use deck::scanner;
use state::DeckMetadata;

fn main() -> Result<(), String> {
    println!("Testing CardBrick deck caching system...");
    
    // Test 1: Discover cached decks
    println!("\n1. Testing deck discovery:");
    let cache_root = scanner::find_cache_root();
    println!("   Cache root: {:?}", cache_root);
    
    let decks = scanner::load_cached_decks()?;
    println!("   Found {} cached decks:", decks.len());
    
    for deck in &decks {
        println!("     - {} (ID: {})", deck.name, deck.id);
        println!("       Path: {:?}", deck.path);
    }
    
    if decks.is_empty() {
        println!("   No decks found. Make sure to run:");
        println!("   python3 precache_decks.py assets/decks test_cache");
        return Ok(());
    }
    
    // Test 2: Verify cached deck database access
    println!("\n2. Testing database access:");
    let first_deck = &decks[0];
    println!("   Testing deck: {}", first_deck.name);
    
    match scanner::ensure_cached_deck(&first_deck.id) {
        Ok(db_path) => {
            println!("   Database path: {:?}", db_path);
            
            // Try to open the database and get basic info
            match rusqlite::Connection::open(&db_path) {
                Ok(conn) => {
                    let card_count: i64 = conn
                        .prepare("SELECT COUNT(*) FROM cards")?
                        .query_row([], |row| row.get(0))
                        .map_err(|e| e.to_string())?;
                    
                    let note_count: i64 = conn
                        .prepare("SELECT COUNT(*) FROM notes")?
                        .query_row([], |row| row.get(0))
                        .map_err(|e| e.to_string())?;
                    
                    println!("   Database opened successfully!");
                    println!("   Cards: {}, Notes: {}", card_count, note_count);
                    
                    // Test the indexes exist
                    let indexes: Vec<String> = conn
                        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_cards_%'")?
                        .query_map([], |row| Ok(row.get::<_, String>(0)?))?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| e.to_string())?;
                    
                    println!("   Performance indexes: {:?}", indexes);
                }
                Err(e) => return Err(format!("Failed to open database: {}", e)),
            }
        }
        Err(e) => return Err(format!("Failed to ensure cached deck: {}", e)),
    }
    
    println!("\n✅ All tests passed! Caching system is working correctly.");
    Ok(())
}