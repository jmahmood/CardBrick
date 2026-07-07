use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A deck in the KARTA JSON format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KartaDeck {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub fields: Vec<String>,
    pub cards: Vec<KartaCard>,
    #[serde(default)]
    pub media: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single card in the KARTA format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KartaCard {
    pub id: String,
    pub fields: HashMap<String, String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl KartaDeck {
    /// Load a deck from a JSON file
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let deck: KartaDeck = serde_json::from_str(&content)?;
        Ok(deck)
    }

    /// Save the deck to a JSON file
    pub fn to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl KartaCard {
    /// Get the front field (typically the word/cue)
    pub fn front(&self) -> Option<&str> {
        self.fields.get("front").map(|s| s.as_str())
    }

    /// Get the back field (typically the definition/answer)
    pub fn back(&self) -> Option<&str> {
        self.fields.get("back").map(|s| s.as_str())
    }

    /// Parse the back field to extract components
    /// Returns (definition, example) if possible
    pub fn parse_back(&self) -> (String, Option<String>) {
        if let Some(back) = self.back() {
            // Try to split on "example from book:" or similar markers
            if let Some(pos) = back.find("example from book:") {
                let definition = back[..pos].trim().to_string();
                let example = back[pos..].trim().to_string();
                return (definition, Some(example));
            } else if let Some(pos) = back.find("example:") {
                let definition = back[..pos].trim().to_string();
                let example = back[pos..].trim().to_string();
                return (definition, Some(example));
            }
            // If no example marker, treat the semicolon as separator
            if let Some(pos) = back.find(';') {
                let definition = back[..pos].trim().to_string();
                let example = back[pos + 1..].trim().to_string();
                return (definition, Some(example));
            }
            // Otherwise, return the whole thing as definition
            (back.to_string(), None)
        } else {
            (String::new(), None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_back_with_example() {
        let mut card = KartaCard {
            id: "test".to_string(),
            fields: HashMap::new(),
            tags: vec![],
            created_at: Utc::now(),
        };

        card.fields.insert(
            "back".to_string(),
            "to shake suddenly; example from book: 'He saw the first tree shudder and fall, far off in the distance.'".to_string()
        );

        let (definition, example) = card.parse_back();
        assert_eq!(definition, "to shake suddenly");
        assert!(example.is_some());
        assert!(example.unwrap().contains("shudder"));
    }

    #[test]
    fn test_parse_back_without_example() {
        let mut card = KartaCard {
            id: "test".to_string(),
            fields: HashMap::new(),
            tags: vec![],
            created_at: Utc::now(),
        };

        card.fields
            .insert("back".to_string(), "to shake suddenly".to_string());

        let (definition, example) = card.parse_back();
        assert_eq!(definition, "to shake suddenly");
        assert!(example.is_none());
    }
}
