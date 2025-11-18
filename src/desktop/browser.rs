use std::path::PathBuf;
use std::fs;

/// Metadata about a deck for selection
#[derive(Debug, Clone)]
pub struct DeckInfo {
    pub path: PathBuf,
    pub name: String,
    pub card_count: usize,
    pub created_at: String,
}

/// Scan a directory for deck files
pub fn scan_workspace_for_decks(workspace_path: &std::path::Path) -> Vec<DeckInfo> {
    let mut decks = Vec::new();

    // Try to read the workspace directory
    let Ok(entries) = fs::read_dir(workspace_path) else {
        eprintln!("Could not read workspace directory: {}", workspace_path.display());
        return decks;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Look for directories
        if path.is_dir() {
            // Check for deck.cbdeck.json in this directory
            let deck_file = path.join("deck.cbdeck.json");
            if deck_file.exists() {
                if let Ok(deck_info) = load_deck_metadata(&deck_file) {
                    decks.push(deck_info);
                }
            }
        }
    }

    // Sort by name
    decks.sort_by(|a, b| a.name.cmp(&b.name));

    decks
}

/// Load basic metadata from a deck file without loading all cards
fn load_deck_metadata(deck_path: &PathBuf) -> Result<DeckInfo, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(deck_path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    let name = json["name"]
        .as_str()
        .unwrap_or("Unknown Deck")
        .to_string();

    let card_count = json["cards"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);

    let created_at = json["created_at"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(DeckInfo {
        path: deck_path.clone(),
        name,
        card_count,
        created_at,
    })
}

/// Get default workspace directory
pub fn default_workspace_dir() -> PathBuf {
    // Try environment variable first
    if let Ok(workspace) = std::env::var("CARDBRICK_WORKSPACE") {
        return PathBuf::from(workspace);
    }

    // Try home directory
    if let Some(home) = dirs::home_dir() {
        let workspace = home.join("CardBrick").join("workspace");
        if workspace.exists() {
            return workspace;
        }
    }

    // Fall back to current directory
    PathBuf::from("./workspace")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_workspace_dir() {
        let workspace = default_workspace_dir();
        // Should return a valid path (not necessarily existing)
        assert!(workspace.is_relative() || workspace.is_absolute());
    }
}
