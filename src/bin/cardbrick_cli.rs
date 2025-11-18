// src/bin/cardbrick_cli.rs
// CardBrick CLI - Command-line interface for data operations

use anyhow::{Result, Context, bail};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::path::PathBuf;
use uuid::Uuid;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "cardbrick-cli")]
#[command(about = "CardBrick command-line interface for data operations")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(short, long, default_value = "json")]
    format: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Deck management operations
    Deck {
        #[command(subcommand)]
        action: DeckCommands,
    },
    /// Study session operations
    Session {
        #[command(subcommand)]
        action: SessionCommands,
    },
    /// Card operations
    Card {
        #[command(subcommand)]
        action: CardCommands,
    },
    /// Progress tracking operations
    Progress {
        #[command(subcommand)]
        action: ProgressCommands,
    },
    /// Statistics operations
    Stats {
        #[command(subcommand)]
        action: StatsCommands,
    },
}

#[derive(Subcommand)]
enum DeckCommands {
    /// List available cached decks
    List,
    /// Show deck information
    Info {
        /// Deck ID (SHA256 hash)
        deck_id: String,
    },
    /// Get cards from deck
    Cards {
        /// Deck ID (SHA256 hash)
        deck_id: String,
        /// Number of cards to return (default: 10)
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Get note content
    Note {
        /// Deck ID (SHA256 hash)
        deck_id: String,
        /// Note ID
        note_id: i64,
    },
}

#[derive(Subcommand)]
enum SessionCommands {
    /// Start a new study session
    Start {
        /// Deck ID (SHA256 hash)
        deck_id: String,
    },
    /// Get next card in session
    Next {
        /// Session ID
        session_id: String,
    },
    /// Rate a card in session
    Rate {
        /// Session ID
        session_id: String,
        /// Card ID
        card_id: i64,
        /// Rating (again, hard, good, easy)
        rating: String,
    },
    /// Get session status
    Status {
        /// Session ID
        session_id: String,
    },
    /// End session
    End {
        /// Session ID
        session_id: String,
    },
}

#[derive(Subcommand)]
enum CardCommands {
    /// Get card and note data
    Get {
        /// Deck ID (SHA256 hash)
        deck_id: String,
        /// Card ID
        card_id: i64,
    },
    /// Apply rating to card using SM-2
    Rate {
        /// Card ID
        card_id: i64,
        /// Rating (again, hard, good, easy)
        rating: String,
    },
    /// Get card review history
    History {
        /// Card ID
        card_id: i64,
    },
}

#[derive(Subcommand)]
enum ProgressCommands {
    /// Get today's progress data
    Daily,
    /// Get daily streak information
    Streak,
    /// Get points information
    Points {
        /// Date (YYYY-MM-DD, default: today)
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Get today's difficult cards
    Difficult,
}

#[derive(Subcommand)]
enum StatsCommands {
    /// Get user profile data
    Profile,
    /// Get deck-specific statistics
    Deck {
        /// Deck ID (SHA256 hash)
        deck_id: String,
    },
    /// Get points breakdown
    Points {
        /// Date (YYYY-MM-DD, default: today)
        #[arg(short, long)]
        date: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Card {
    pub id: i64,
    pub note_id: i64,
    pub due: i64,
    pub interval: u32,
    pub ease_factor: u32,
    pub lapses: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Note {
    pub id: i64,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeckMetadata {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeckManifest {
    pub apkg_name: String,
    pub sha256: String,
    pub created_at: String,
    pub db_file: String,
    pub card_count: u64,
    pub notes_count: u64,
    pub deck_name: String,
    pub anki_version: u32,
}

/// Session state for managing active study sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionState {
    id: String,
    deck_id: String,
    deck_path: PathBuf,
    created_at: chrono::DateTime<chrono::Utc>,
    cards_studied: usize,
    current_card: Option<Card>,
}

impl SessionState {
    fn new(deck_id: String, deck_path: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            deck_id,
            deck_path,
            created_at: chrono::Utc::now(),
            cards_studied: 0,
            current_card: None,
        }
    }
}

/// Rating enum for SM-2 algorithm
#[derive(Debug, Clone, Copy, PartialEq)]
enum Rating {
    Again,
    Hard,
    Good,
    Easy,
}

/// Convert CLI rating string to Rating enum
fn parse_rating(rating_str: &str) -> Result<Rating> {
    match rating_str.to_lowercase().as_str() {
        "again" => Ok(Rating::Again),
        "hard" => Ok(Rating::Hard),
        "good" => Ok(Rating::Good),
        "easy" => Ok(Rating::Easy),
        _ => bail!("Invalid rating: {}. Must be one of: again, hard, good, easy", rating_str),
    }
}

/// Output result as JSON
fn output_json(result: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(result)?);
    Ok(())
}

/// Output error as JSON
fn output_error(error: &str) -> Result<()> {
    let error_obj = serde_json::json!({
        "error": true,
        "message": error
    });
    println!("{}", serde_json::to_string_pretty(&error_obj)?);
    Ok(())
}

/// Find the cache root directory based on the platform (copied from scanner logic)
fn find_cache_root() -> PathBuf {
    if std::path::Path::new("/storage").exists() {
        // RG35XX Plus - use storage partition
        PathBuf::from("/storage/applications/CardBrick/decks")
    } else if std::path::Path::new("/mnt/SDCARD").exists() {
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
fn load_manifest(manifest_path: &std::path::Path) -> Result<DeckManifest> {
    let content = std::fs::read_to_string(manifest_path)
        .context("Failed to read manifest file")?;

    serde_json::from_str(&content)
        .context("Failed to parse manifest JSON")
}

/// Load cached decks from the cache directory (simplified version of scanner logic)
fn load_cached_decks() -> Result<Vec<DeckMetadata>> {
    let cache_root = find_cache_root();

    if !cache_root.exists() {
        return Ok(Vec::new());
    }

    let mut decks = Vec::new();
    let entries = std::fs::read_dir(&cache_root)
        .context("Failed to read cache directory")?;

    for entry in entries {
        let entry = entry.context("Failed to read cache directory entry")?;
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

/// Get deck path from deck ID
fn get_deck_path(deck_id: &str) -> Result<PathBuf> {
    let cached_decks = load_cached_decks()
        .context("Failed to load cached decks")?;

    let deck_metadata = cached_decks.iter()
        .find(|d| d.id == deck_id)
        .context("Deck not found")?;

    Ok(deck_metadata.path.clone())
}

/// Get deck database path
fn get_deck_db_path(deck_id: &str) -> Result<PathBuf> {
    let deck_path = get_deck_path(deck_id)?;
    let manifest_path = deck_path.join("manifest.json");
    let manifest = load_manifest(&manifest_path)?;
    Ok(deck_path.join(&manifest.db_file))
}

/// Load cards from deck database
fn load_cards_from_deck(deck_id: &str, limit: Option<usize>) -> Result<Vec<Card>> {
    let db_path = get_deck_db_path(deck_id)?;
    let conn = Connection::open(&db_path)?;

    let query = if let Some(limit) = limit {
        format!("SELECT id, nid, due, ivl, factor, lapses FROM cards LIMIT {}", limit)
    } else {
        "SELECT id, nid, due, ivl, factor, lapses FROM cards".to_string()
    };

    let mut stmt = conn.prepare(&query)?;
    let card_iter = stmt.query_map([], |row| {
        Ok(Card {
            id: row.get(0)?,
            note_id: row.get(1)?,
            due: row.get(2)?,
            interval: row.get(3)?,
            ease_factor: row.get(4)?,
            lapses: row.get(5)?,
        })
    })?;

    let mut cards = Vec::new();
    for card in card_iter {
        cards.push(card?);
    }

    Ok(cards)
}

/// Load note from deck database
fn load_note_from_deck(deck_id: &str, note_id: i64) -> Result<Option<Note>> {
    let db_path = get_deck_db_path(deck_id)?;
    let conn = Connection::open(&db_path)?;

    let note: Option<Note> = conn.query_row(
        "SELECT id, flds FROM notes WHERE id = ?",
        [note_id],
        |row| {
            let id: i64 = row.get(0)?;
            let fields_str: String = row.get(1)?;
            let fields: Vec<String> = fields_str.split('\x1f').map(String::from).collect();
            Ok(Note { id, fields })
        }
    ).optional()?;

    Ok(note)
}

/// Get progress database path
fn progress_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cardbrick")
        .join("progress.db")
}

/// Apply SM-2 rating algorithm (copied from main app)
fn apply_sm2_rating(card_id: i64, quality: Rating, timestamp: i64) -> Result<()> {
    let db_path = progress_path();

    // Ensure progress database exists and is initialized
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(&db_path)?;

    // Initialize the required tables if they don't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS srs_log (
            card_id INTEGER PRIMARY KEY,
            next_due_ts INTEGER,
            interval INTEGER,
            ease_factor REAL,
            reps INTEGER,
            lapses INTEGER
        )",
        [],
    )?;

    // Get current card state from srs_log, or create new entry
    let mut stmt = conn.prepare(
        "SELECT interval, ease_factor, reps, lapses FROM srs_log WHERE card_id = ?1"
    )?;

    let (mut interval, mut ease_factor, mut reps, mut lapses): (u32, f32, u32, u32) = match stmt.query_row([card_id], |row| {
        let raw_interval: Option<i64> = row.get(0)?;
        let raw_ease: Option<f64> = row.get(1)?;
        let raw_reps: Option<i64> = row.get(2)?;
        let raw_lapses: Option<i64> = row.get(3)?;

        let interval = match raw_interval {
            Some(v) if v > 0 => (v as u64).min(36500) as u32, // Cap at ~100 years
            _ => 0,
        };
        let ease_factor = match raw_ease {
            Some(v) => (v as f32).clamp(1.3, 2.5),
            None => 2.5,
        };
        let reps = match raw_reps {
            Some(v) if v > 0 => v as u32,
            _ => 0,
        };
        let lapses = match raw_lapses {
            Some(v) if v > 0 => v as u32,
            _ => 0,
        };

        Ok((interval, ease_factor, reps, lapses))
    }) {
        Ok(values) => values,
        Err(_) => (0, 2.5, 0, 0), // New card defaults
    };

    // Apply SM-2 algorithm based on quality rating
    match quality {
        Rating::Again => {
            lapses += 1;
            ease_factor = (ease_factor - 0.2).max(1.3);
            interval = 1;
            reps = 0;
        }
        Rating::Hard => {
            ease_factor = (ease_factor - 0.15).max(1.3);
            reps += 1;
            interval = if interval == 0 {
                1
            } else {
                ((interval as f32 * 1.2).round() as u32).max(interval + 1)
            };
        }
        Rating::Good => {
            reps += 1;
            interval = if interval == 0 {
                1
            } else if interval == 1 {
                6
            } else {
                (interval as f32 * ease_factor).round() as u32
            };
        }
        Rating::Easy => {
            ease_factor = (ease_factor + 0.15).min(2.5);
            reps += 1;
            interval = if interval == 0 {
                4
            } else {
                (interval as f32 * ease_factor * 1.3).round() as u32
            };
        }
    }

    // Calculate next due timestamp
    let next_due_ts = timestamp + (interval as i64 * 86400); // days to seconds

    // Update or insert the card state
    conn.execute(
        "INSERT OR REPLACE INTO srs_log (card_id, next_due_ts, interval, ease_factor, reps, lapses)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (card_id, next_due_ts, interval as i64, ease_factor as f64, reps as i64, lapses as i64),
    )?;

    Ok(())
}

/// Initialize progress database with required tables
fn init_progress_database() -> Result<()> {
    let db_path = progress_path();

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(&db_path)?;

    // SRS log table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS srs_log (
            card_id INTEGER PRIMARY KEY,
            next_due_ts INTEGER,
            interval INTEGER,
            ease_factor REAL,
            reps INTEGER,
            lapses INTEGER
        )",
        [],
    )?;

    // Daily ratings table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_ratings (
            card_id INTEGER,
            rating TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            date TEXT NOT NULL,
            PRIMARY KEY (card_id, timestamp)
        )",
        [],
    )?;

    // Difficult cards table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS difficult_cards (
            card_id INTEGER,
            difficulty_type TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            date TEXT NOT NULL,
            PRIMARY KEY (card_id, difficulty_type, date)
        )",
        [],
    )?;

    // Profile table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS profile (
            id INTEGER PRIMARY KEY DEFAULT 1,
            total_score INTEGER DEFAULT 0,
            level_score INTEGER DEFAULT 0,
            daily_streak INTEGER DEFAULT 0,
            last_study_date TEXT,
            CHECK (id = 1)
        )",
        [],
    )?;

    // Study events table for points
    conn.execute(
        "CREATE TABLE IF NOT EXISTS study_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL,
            timestamp INTEGER NOT NULL,
            bp INTEGER NOT NULL,
            df INTEGER NOT NULL,
            cb INTEGER NOT NULL,
            sb INTEGER NOT NULL,
            pa INTEGER NOT NULL
        )",
        [],
    )?;

    Ok(())
}

/// Session management file operations
fn get_session_dir() -> PathBuf {
    std::env::temp_dir().join("cardbrick-sessions")
}

fn ensure_session_dir() -> Result<()> {
    let session_dir = get_session_dir();
    std::fs::create_dir_all(&session_dir)
        .context("Failed to create session directory")?;
    Ok(())
}

fn save_session(session: &SessionState) -> Result<()> {
    ensure_session_dir()?;
    let session_file = get_session_dir().join(format!("{}.json", session.id));
    let session_json = serde_json::to_string_pretty(session)?;
    std::fs::write(&session_file, session_json)
        .context("Failed to save session")?;
    Ok(())
}

fn load_session(session_id: &str) -> Result<SessionState> {
    let session_file = get_session_dir().join(format!("{}.json", session_id));
    let session_json = std::fs::read_to_string(&session_file)
        .context("Failed to read session file")?;
    let session: SessionState = serde_json::from_str(&session_json)
        .context("Failed to parse session")?;
    Ok(session)
}

fn delete_session(session_id: &str) -> Result<()> {
    let session_file = get_session_dir().join(format!("{}.json", session_id));
    if session_file.exists() {
        std::fs::remove_file(&session_file)
            .context("Failed to delete session file")?;
    }
    Ok(())
}

/// Handle deck commands
fn handle_deck_command(action: DeckCommands, _verbose: bool) -> Result<()> {
    match action {
        DeckCommands::List => {
            let cached_decks = load_cached_decks()
                .context("Failed to load cached decks")?;

            let result = serde_json::json!({
                "decks": cached_decks.iter().map(|deck| {
                    serde_json::json!({
                        "id": deck.id,
                        "name": deck.name,
                        "path": deck.path
                    })
                }).collect::<Vec<_>>()
            });

            output_json(&result)
        }

        DeckCommands::Info { deck_id } => {
            let cached_decks = load_cached_decks()
                .context("Failed to load cached decks")?;

            let deck_metadata = cached_decks.iter()
                .find(|d| d.id == deck_id)
                .context("Deck not found")?;

            // Load manifest for additional info
            let manifest_path = deck_metadata.path.join("manifest.json");
            let manifest_content = std::fs::read_to_string(&manifest_path)
                .context("Failed to read manifest")?;
            let manifest: serde_json::Value = serde_json::from_str(&manifest_content)
                .context("Failed to parse manifest")?;

            let result = serde_json::json!({
                "id": deck_metadata.id,
                "name": deck_metadata.name,
                "path": deck_metadata.path,
                "manifest": manifest
            });

            output_json(&result)
        }

        DeckCommands::Cards { deck_id, limit } => {
            let cards = load_cards_from_deck(&deck_id, Some(limit))?;

            let result = serde_json::json!({
                "deck_id": deck_id,
                "cards": cards,
                "count": cards.len()
            });

            output_json(&result)
        }

        DeckCommands::Note { deck_id, note_id } => {
            let note = load_note_from_deck(&deck_id, note_id)?;

            let result = serde_json::json!({
                "deck_id": deck_id,
                "note": note
            });

            output_json(&result)
        }
    }
}

/// Handle session commands
fn handle_session_command(action: SessionCommands, _verbose: bool) -> Result<()> {
    match action {
        SessionCommands::Start { deck_id } => {
            let deck_path = get_deck_path(&deck_id)?;
            let session = SessionState::new(deck_id.clone(), deck_path);
            save_session(&session)?;

            let result = serde_json::json!({
                "session_id": session.id,
                "deck_id": deck_id,
                "created_at": session.created_at,
                "status": "started"
            });

            output_json(&result)
        }

        SessionCommands::Next { session_id } => {
            let mut session = load_session(&session_id)?;

            // Initialize progress database if needed
            init_progress_database()?;
            let db_path = progress_path();
            let conn = Connection::open(&db_path)?;

            // Get all cards from the deck
            let all_cards = load_cards_from_deck(&session.deck_id, None)?;

            // Smart card selection: prioritize due cards, then new cards
            let now_ts = chrono::Utc::now().timestamp();
            let mut selected_card = None;

            // Step 1: Look for due cards first
            for card in &all_cards {
                let due_ts: Option<i64> = conn
                    .prepare("SELECT next_due_ts FROM srs_log WHERE card_id = ?1")?
                    .query_row([card.id], |row| Ok(row.get(0)?))
                    .optional()?;

                match due_ts {
                    Some(due) if due <= now_ts => {
                        // This card is due for review
                        selected_card = Some(card.clone());
                        break;
                    }
                    None => {
                        // This is a new card (not in SRS yet)
                        selected_card = Some(card.clone());
                        break;
                    }
                    _ => {
                        // Card not yet due, continue looking
                    }
                }
            }

            // Step 2: If no due or new cards, pick any card
            if selected_card.is_none() && !all_cards.is_empty() {
                // Pick the card with the earliest due date
                let mut earliest_card = None;
                let mut earliest_due = i64::MAX;

                for card in &all_cards {
                    let due_ts: Option<i64> = conn
                        .prepare("SELECT next_due_ts FROM srs_log WHERE card_id = ?1")?
                        .query_row([card.id], |row| Ok(row.get(0)?))
                        .optional()?;

                    let due = due_ts.unwrap_or(0);
                    if due < earliest_due {
                        earliest_due = due;
                        earliest_card = Some(card.clone());
                    }
                }

                selected_card = earliest_card;
            }

            session.current_card = selected_card.clone();
            save_session(&session)?;

            let result = match selected_card {
                Some(c) => {
                    // Check if this card is due or new
                    let due_ts: Option<i64> = conn
                        .prepare("SELECT next_due_ts FROM srs_log WHERE card_id = ?1")?
                        .query_row([c.id], |row| Ok(row.get(0)?))
                        .optional()?;

                    let card_status = match due_ts {
                        Some(due) if due <= now_ts => "due",
                        Some(_) => "future",
                        None => "new",
                    };

                    serde_json::json!({
                        "session_id": session_id,
                        "card": c,
                        "card_status": card_status,
                        "has_more": true
                    })
                }
                None => serde_json::json!({
                    "session_id": session_id,
                    "card": null,
                    "has_more": false,
                    "session_complete": true
                })
            };

            output_json(&result)
        }

        SessionCommands::Rate { session_id, card_id, rating } => {
            let mut session = load_session(&session_id)?;
            let rating_enum = parse_rating(&rating)?;
            let timestamp = chrono::Utc::now().timestamp();

            // Initialize progress database if needed
            init_progress_database()?;

            // Apply real SM-2 algorithm
            apply_sm2_rating(card_id, rating_enum, timestamp)?;

            // Record daily rating and difficult cards
            let db_path = progress_path();
            let conn = Connection::open(&db_path)?;
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

            conn.execute(
                "INSERT OR REPLACE INTO daily_ratings (card_id, rating, timestamp, date)
                 VALUES (?1, ?2, ?3, ?4)",
                (card_id, &rating, timestamp, &today),
            )?;

            if matches!(rating_enum, Rating::Hard | Rating::Again) {
                let difficulty_type = match rating_enum {
                    Rating::Again => "failed",
                    Rating::Hard => "hard",
                    _ => unreachable!(),
                };

                conn.execute(
                    "INSERT OR REPLACE INTO difficult_cards (card_id, difficulty_type, timestamp, date)
                     VALUES (?1, ?2, ?3, ?4)",
                    (card_id, difficulty_type, timestamp, &today),
                )?;
            }

            session.cards_studied += 1;
            save_session(&session)?;

            let result = serde_json::json!({
                "session_id": session_id,
                "card_id": card_id,
                "rating": rating,
                "cards_studied": session.cards_studied,
                "status": "rated",
                "timestamp": timestamp
            });

            output_json(&result)
        }

        SessionCommands::Status { session_id } => {
            let session = load_session(&session_id)?;

            let result = serde_json::json!({
                "session_id": session.id,
                "deck_id": session.deck_id,
                "created_at": session.created_at,
                "cards_studied": session.cards_studied,
                "current_card": session.current_card
            });

            output_json(&result)
        }

        SessionCommands::End { session_id } => {
            let session = load_session(&session_id)?;
            delete_session(&session_id)?;

            let result = serde_json::json!({
                "session_id": session.id,
                "cards_studied": session.cards_studied,
                "duration_minutes": (chrono::Utc::now() - session.created_at).num_minutes(),
                "status": "ended"
            });

            output_json(&result)
        }
    }
}

/// Handle card commands
fn handle_card_command(action: CardCommands, _verbose: bool) -> Result<()> {
    match action {
        CardCommands::Get { deck_id, card_id } => {
            let cards = load_cards_from_deck(&deck_id, None)?;
            let card = cards.iter()
                .find(|c| c.id == card_id)
                .context("Card not found")?;

            let note = load_note_from_deck(&deck_id, card.note_id)?;

            let result = serde_json::json!({
                "deck_id": deck_id,
                "card": card,
                "note": note
            });

            output_json(&result)
        }

        CardCommands::Rate { card_id, rating } => {
            let rating_enum = parse_rating(&rating)?;
            let timestamp = chrono::Utc::now().timestamp();

            // Initialize progress database if needed
            init_progress_database()?;

            // Apply real SM-2 algorithm
            apply_sm2_rating(card_id, rating_enum, timestamp)?;

            // Record daily rating for progress tracking
            let db_path = progress_path();
            let conn = Connection::open(&db_path)?;
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

            conn.execute(
                "INSERT OR REPLACE INTO daily_ratings (card_id, rating, timestamp, date)
                 VALUES (?1, ?2, ?3, ?4)",
                (card_id, &rating, timestamp, &today),
            )?;

            // Record difficult cards if hard or again
            if matches!(rating_enum, Rating::Hard | Rating::Again) {
                let difficulty_type = match rating_enum {
                    Rating::Again => "failed",
                    Rating::Hard => "hard",
                    _ => unreachable!(),
                };

                conn.execute(
                    "INSERT OR REPLACE INTO difficult_cards (card_id, difficulty_type, timestamp, date)
                     VALUES (?1, ?2, ?3, ?4)",
                    (card_id, difficulty_type, timestamp, &today),
                )?;
            }

            // Get updated card state to return
            let srs_state: Option<(i64, f64, i64, i64)> = conn.query_row(
                "SELECT interval, ease_factor, reps, lapses FROM srs_log WHERE card_id = ?1",
                [card_id],
                |row| Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            ).optional()?;

            let result = serde_json::json!({
                "card_id": card_id,
                "rating": rating,
                "timestamp": timestamp,
                "status": "rated",
                "srs_state": srs_state.map(|(interval, ease, reps, lapses)| {
                    serde_json::json!({
                        "interval": interval,
                        "ease_factor": ease,
                        "repetitions": reps,
                        "lapses": lapses
                    })
                })
            });

            output_json(&result)
        }

        CardCommands::History { card_id } => {
            // Try to get history from the progress database first
            init_progress_database()?;
            let db_path = progress_path();
            let conn = Connection::open(&db_path)?;

            let mut history = Vec::new();

            // Get ratings from daily_ratings table
            let mut stmt = conn.prepare(
                "SELECT rating, timestamp, date FROM daily_ratings
                 WHERE card_id = ?1
                 ORDER BY timestamp DESC"
            )?;

            let rating_iter = stmt.query_map([card_id], |row| {
                Ok(serde_json::json!({
                    "rating": row.get::<_, String>(0)?,
                    "timestamp": row.get::<_, i64>(1)?,
                    "date": row.get::<_, String>(2)?,
                    "source": "daily_ratings"
                }))
            })?;

            for rating in rating_iter {
                history.push(rating?);
            }

            // If no history in progress DB, try to find deck and look in revlog
            if history.is_empty() {
                // We need to find which deck this card belongs to
                let cached_decks = load_cached_decks()?;
                for deck_metadata in cached_decks {
                    let db_path = get_deck_db_path(&deck_metadata.id)?;
                    if let Ok(deck_conn) = Connection::open(&db_path) {
                        // Check if this card exists in this deck
                        let card_exists: bool = deck_conn
                            .prepare("SELECT 1 FROM cards WHERE id = ?1")?
                            .query_row([card_id], |_| Ok(true))
                            .unwrap_or(false);

                        if card_exists {
                            // Get revlog entries for this card
                            let mut revlog_stmt = deck_conn.prepare(
                                "SELECT id, cid, usn, ease, ivl, lastIvl, factor, time, type
                                 FROM revlog
                                 WHERE cid = ?1
                                 ORDER BY id DESC"
                            ).unwrap_or_else(|_| {
                                // If revlog table doesn't exist, create empty prepared statement
                                deck_conn.prepare("SELECT 1 WHERE 0").unwrap()
                            });

                            if let Ok(revlog_iter) = revlog_stmt.query_map([card_id], |row| {
                                Ok(serde_json::json!({
                                    "id": row.get::<_, i64>(0)?,
                                    "card_id": row.get::<_, i64>(1)?,
                                    "ease": row.get::<_, Option<i32>>(3)?,
                                    "interval": row.get::<_, Option<i32>>(4)?,
                                    "last_interval": row.get::<_, Option<i32>>(5)?,
                                    "factor": row.get::<_, Option<i32>>(6)?,
                                    "time_taken": row.get::<_, Option<i32>>(7)?,
                                    "review_type": row.get::<_, Option<i32>>(8)?,
                                    "source": "revlog"
                                }))
                            }) {
                                for entry in revlog_iter {
                                    if let Ok(entry) = entry {
                                        history.push(entry);
                                    }
                                }
                            }
                            break; // Found the right deck
                        }
                    }
                }
            }

            let result = serde_json::json!({
                "card_id": card_id,
                "history": history,
                "total_reviews": history.len()
            });

            output_json(&result)
        }
    }
}

/// Handle progress commands
fn handle_progress_command(action: ProgressCommands, _verbose: bool) -> Result<()> {
    // Initialize progress database if needed
    init_progress_database()?;
    let db_path = progress_path();
    let conn = Connection::open(&db_path)?;

    match action {
        ProgressCommands::Daily => {
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

            // Get today's ratings
            let mut stmt = conn.prepare(
                "SELECT card_id, rating FROM daily_ratings
                 WHERE date = ?1
                 ORDER BY timestamp ASC"
            )?;

            let rating_iter = stmt.query_map([&today], |row| {
                Ok((
                    row.get::<_, i64>(0)?,    // card_id
                    row.get::<_, String>(1)?, // rating
                ))
            })?;

            let mut ratings = Vec::new();
            for rating in rating_iter {
                ratings.push(rating?);
            }

            // Get total points for today (simplified calculation)
            let today_start = chrono::Utc::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp();
            let today_end = today_start + 86400;

            let points: i32 = conn
                .prepare("SELECT COALESCE(SUM(pa), 0) FROM study_events WHERE timestamp >= ?1 AND timestamp < ?2")?
                .query_row([today_start, today_end], |row| Ok(row.get(0)?))
                .unwrap_or(0);

            let result = serde_json::json!({
                "date": today,
                "ratings": ratings,
                "total_points": points,
                "cards_studied": ratings.len()
            });

            output_json(&result)
        }

        ProgressCommands::Streak => {
            // Initialize profile if it doesn't exist
            conn.execute(
                "INSERT OR IGNORE INTO profile (id, total_score, level_score, daily_streak, last_study_date)
                 VALUES (1, 0, 0, 0, NULL)",
                [],
            )?;

            let (total_score, level_score, daily_streak, last_study_date) = conn
                .prepare("SELECT total_score, level_score, daily_streak, last_study_date FROM profile WHERE id = 1")?
                .query_row([], |row| {
                    Ok((
                        row.get::<_, i32>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, i32>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })?;

            let result = serde_json::json!({
                "daily_streak": daily_streak,
                "last_study_date": last_study_date,
                "total_score": total_score,
                "level_score": level_score
            });

            output_json(&result)
        }

        ProgressCommands::Points { date } => {
            let target_date = date.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

            // Parse the target date and get timestamp range
            let parsed_date = chrono::NaiveDate::parse_from_str(&target_date, "%Y-%m-%d")
                .context("Invalid date format, use YYYY-MM-DD")?;
            let start_ts = parsed_date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
            let end_ts = start_ts + 86400;

            let points: i32 = conn
                .prepare("SELECT COALESCE(SUM(pa), 0) FROM study_events WHERE timestamp >= ?1 AND timestamp < ?2")?
                .query_row([start_ts, end_ts], |row| Ok(row.get(0)?))
                .unwrap_or(0);

            let result = serde_json::json!({
                "date": target_date,
                "total_points": points
            });

            output_json(&result)
        }

        ProgressCommands::Difficult => {
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

            // Get failed cards for today
            let mut stmt = conn.prepare(
                "SELECT card_id FROM difficult_cards
                 WHERE date = ?1 AND difficulty_type = 'failed'
                 ORDER BY timestamp DESC"
            )?;

            let failed_iter = stmt.query_map([&today], |row| {
                Ok(row.get::<_, i64>(0)?)
            })?;

            let mut failed_cards = Vec::new();
            for card in failed_iter {
                failed_cards.push(card?);
            }

            // Get hard cards for today
            let mut stmt = conn.prepare(
                "SELECT card_id FROM difficult_cards
                 WHERE date = ?1 AND difficulty_type = 'hard'
                 ORDER BY timestamp DESC"
            )?;

            let hard_iter = stmt.query_map([&today], |row| {
                Ok(row.get::<_, i64>(0)?)
            })?;

            let mut hard_cards = Vec::new();
            for card in hard_iter {
                hard_cards.push(card?);
            }

            let result = serde_json::json!({
                "date": today,
                "failed_cards": failed_cards,
                "hard_cards": hard_cards,
                "total_difficult": failed_cards.len() + hard_cards.len()
            });

            output_json(&result)
        }
    }
}

/// Handle statistics commands
fn handle_stats_command(action: StatsCommands, _verbose: bool) -> Result<()> {
    // Initialize progress database if needed
    init_progress_database()?;
    let db_path = progress_path();
    let conn = Connection::open(&db_path)?;

    match action {
        StatsCommands::Profile => {
            // Initialize profile if it doesn't exist
            conn.execute(
                "INSERT OR IGNORE INTO profile (id, total_score, level_score, daily_streak, last_study_date)
                 VALUES (1, 0, 0, 0, NULL)",
                [],
            )?;

            let (total_score, level_score, daily_streak, last_study_date) = conn
                .prepare("SELECT total_score, level_score, daily_streak, last_study_date FROM profile WHERE id = 1")?
                .query_row([], |row| {
                    Ok((
                        row.get::<_, i32>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, i32>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })?;

            // Get today's points
            let today_start = chrono::Utc::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp();
            let today_end = today_start + 86400;

            let points_today: i32 = conn
                .prepare("SELECT COALESCE(SUM(pa), 0) FROM study_events WHERE timestamp >= ?1 AND timestamp < ?2")?
                .query_row([today_start, today_end], |row| Ok(row.get(0)?))
                .unwrap_or(0);

            // Get total cards in SRS
            let total_srs_cards: i32 = conn
                .prepare("SELECT COUNT(*) FROM srs_log")?
                .query_row([], |row| Ok(row.get(0)?))
                .unwrap_or(0);

            let result = serde_json::json!({
                "total_score": total_score,
                "level_score": level_score,
                "daily_streak": daily_streak,
                "last_study_date": last_study_date,
                "points_today": points_today,
                "total_srs_cards": total_srs_cards
            });

            output_json(&result)
        }

        StatsCommands::Deck { deck_id } => {
            let cards = load_cards_from_deck(&deck_id, None)?;

            // Get SRS statistics for cards in this deck
            let deck_card_ids: Vec<i64> = cards.iter().map(|c| c.id).collect();
            let card_ids_str = deck_card_ids.iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let srs_stats = if !deck_card_ids.is_empty() {
                let query = format!(
                    "SELECT
                        COUNT(*) as total_in_srs,
                        AVG(ease_factor) as avg_ease,
                        AVG(interval) as avg_interval,
                        SUM(lapses) as total_lapses
                     FROM srs_log
                     WHERE card_id IN ({})",
                    card_ids_str
                );

                conn.query_row(&query, [], |row| {
                    Ok(serde_json::json!({
                        "total_in_srs": row.get::<_, i32>(0)?,
                        "avg_ease_factor": row.get::<_, Option<f64>>(1)?,
                        "avg_interval": row.get::<_, Option<f64>>(2)?,
                        "total_lapses": row.get::<_, Option<i32>>(3)?
                    }))
                }).unwrap_or_else(|_| serde_json::json!({
                    "total_in_srs": 0,
                    "avg_ease_factor": null,
                    "avg_interval": null,
                    "total_lapses": 0
                }))
            } else {
                serde_json::json!({
                    "total_in_srs": 0,
                    "avg_ease_factor": null,
                    "avg_interval": null,
                    "total_lapses": 0
                })
            };

            let result = serde_json::json!({
                "deck_id": deck_id,
                "total_cards": cards.len(),
                "srs_statistics": srs_stats
            });

            output_json(&result)
        }

        StatsCommands::Points { date } => {
            let target_date = date.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

            // Parse the target date and get timestamp range
            let parsed_date = chrono::NaiveDate::parse_from_str(&target_date, "%Y-%m-%d")
                .context("Invalid date format, use YYYY-MM-DD")?;
            let start_ts = parsed_date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
            let end_ts = start_ts + 86400;

            // Get point breakdown from study events
            let breakdown = conn.query_row(
                "SELECT
                    COUNT(*) as total_events,
                    SUM(bp) as base_points,
                    SUM(df) as difficulty_bonus,
                    SUM(cb) as combo_bonus,
                    SUM(sb) as speed_bonus,
                    SUM(pa) as total_points
                 FROM study_events
                 WHERE timestamp >= ?1 AND timestamp < ?2",
                [start_ts, end_ts],
                |row| {
                    Ok(serde_json::json!({
                        "total_events": row.get::<_, i32>(0)?,
                        "base_points": row.get::<_, Option<i32>>(1)?,
                        "difficulty_bonus": row.get::<_, Option<i32>>(2)?,
                        "combo_bonus": row.get::<_, Option<i32>>(3)?,
                        "speed_bonus": row.get::<_, Option<i32>>(4)?,
                        "total_points": row.get::<_, Option<i32>>(5)?
                    }))
                }
            ).unwrap_or_else(|_| serde_json::json!({
                "total_events": 0,
                "base_points": 0,
                "difficulty_bonus": 0,
                "combo_bonus": 0,
                "speed_bonus": 0,
                "total_points": 0
            }));

            let result = serde_json::json!({
                "date": target_date,
                "breakdown": breakdown
            });

            output_json(&result)
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Deck { action } => handle_deck_command(action, cli.verbose),
        Commands::Session { action } => handle_session_command(action, cli.verbose),
        Commands::Card { action } => handle_card_command(action, cli.verbose),
        Commands::Progress { action } => handle_progress_command(action, cli.verbose),
        Commands::Stats { action } => handle_stats_command(action, cli.verbose),
    };

    if let Err(e) = result {
        output_error(&e.to_string())?;
        std::process::exit(1);
    }

    Ok(())
}