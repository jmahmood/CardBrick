// src/storage/connection.rs
// Centralized database connection management to eliminate duplication

use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};
use std::fs;
use crate::storage::db::progress_path;
use crate::storage::schema::*;

/// Opens a connection to the progress database
/// Creates the database and tables if they don't exist
pub fn open_progress_db() -> Result<Connection, Box<dyn std::error::Error>> {
    let path = progress_path();
    
    // Ensure the directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let conn = Connection::open(&path)?;
    create_progress_database_tables(&conn)?;
    Ok(conn)
}

/// Opens a connection to a deck-specific database
/// Creates the database and tables if they don't exist
pub fn open_deck_db(deck_id: &str) -> Result<Connection, Box<dyn std::error::Error>> {
    let path = Path::new("anki/history");
    fs::create_dir_all(path)?;
    let db_path = path.join(format!("{}.db", deck_id));
    
    let conn = Connection::open(&db_path)?;
    create_deck_database_tables(&conn)?;
    Ok(conn)
}

/// Opens a connection to a test database at the specified path
/// Creates the database and all test tables
pub fn open_test_db(path: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    let conn = Connection::open(path)?;
    create_test_database_tables(&conn)?;
    Ok(conn)
}

/// Opens a connection to a custom database with specified tables
/// Useful for specific test scenarios or custom setups
pub fn open_custom_db(path: &Path, table_creator: fn(&Connection) -> Result<()>) -> Result<Connection, Box<dyn std::error::Error>> {
    let conn = Connection::open(path)?;
    table_creator(&conn)?;
    Ok(conn)
}

/// Opens a connection to an existing database without creating tables
/// Use when you know the database already exists and is properly initialized
pub fn open_existing_db(path: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    let conn = Connection::open(path)?;
    Ok(conn)
}

/// Opens a connection with custom path resolution
/// Used by functions that need to override default path logic
pub fn open_db_with_path(path: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    let conn = Connection::open(path)?;
    Ok(conn)
}

/// Creates a database connection with retry logic
/// Attempts to connect multiple times before failing
pub fn open_db_with_retry(path: &Path, max_retries: u32) -> Result<Connection, Box<dyn std::error::Error>> {
    let mut last_error = None;
    
    for attempt in 0..max_retries {
        match Connection::open(path) {
            Ok(conn) => return Ok(conn),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_retries - 1 {
                    std::thread::sleep(std::time::Duration::from_millis(100 * (attempt + 1) as u64));
                }
            }
        }
    }
    
    Err(last_error.unwrap().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_progress_db() {
        let conn = open_progress_db().unwrap();
        
        // Verify tables exist
        let table_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            [],
            |row| row.get(0)
        ).unwrap();
        
        assert!(table_count >= 4, "Should have at least 4 core tables");
    }

    #[test]
    fn test_open_test_db() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let conn = open_test_db(&db_path).unwrap();
        
        // Verify test tables exist
        let table_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            [],
            |row| row.get(0)
        ).unwrap();
        
        assert!(table_count >= 6, "Should have at least 6 test tables");
    }

    #[test]
    fn test_open_custom_db() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("custom.db");
        
        let conn = open_custom_db(&db_path, |conn| {
            create_srs_log_table(conn)?;
            create_cards_table(conn)?;
            Ok(())
        }).unwrap();
        
        // Verify custom tables exist
        let table_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            [],
            |row| row.get(0)
        ).unwrap();
        
        assert_eq!(table_count, 2, "Should have exactly 2 custom tables");
    }

    #[test]
    fn test_open_db_with_retry() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("retry_test.db");
        
        // Should succeed on first try for a valid path
        let conn = open_db_with_retry(&db_path, 3).unwrap();
        drop(conn);
        
        // Test with an invalid path (should fail after retries)
        let invalid_path = Path::new("/invalid/nonexistent/path/test.db");
        let result = open_db_with_retry(invalid_path, 2);
        assert!(result.is_err(), "Should fail for invalid path");
    }
}