use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub column: String,
    pub order: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub archived: bool,
}

impl Card {
    pub fn new(title: impl Into<String>, column: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Ulid::new().to_string(),
            title: title.into(),
            column: column.into(),
            order: 0,
            description: None,
            assignee: None,
            labels: Vec::new(),
            due: None,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
            archived: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_card_has_valid_defaults() {
        let card = Card::new("Test task", "todo");
        assert_eq!(card.title, "Test task");
        assert_eq!(card.column, "todo");
        assert_eq!(card.order, 0);
        assert!(card.description.is_none());
        assert!(card.assignee.is_none());
        assert!(card.labels.is_empty());
        assert!(card.due.is_none());
        assert!(!card.archived);
        assert!(card.metadata.is_empty());
        // ULID is 26 chars
        assert_eq!(card.id.len(), 26);
    }

    #[test]
    fn card_roundtrip_json() {
        let card = Card::new("Roundtrip", "doing");
        let json = serde_json::to_string_pretty(&card).unwrap();
        let deserialized: Card = serde_json::from_str(&json).unwrap();
        assert_eq!(card, deserialized);
    }

    #[test]
    fn card_with_all_fields_roundtrip() {
        let mut card = Card::new("Full card", "review");
        card.description = Some("A description".into());
        card.assignee = Some("leslie".into());
        card.labels = vec!["bug".into(), "urgent".into()];
        card.due = Some(Utc::now());
        card.metadata.insert(
            "pr_url".into(),
            serde_json::json!("https://github.com/pr/1"),
        );
        card.archived = true;

        let json = serde_json::to_string_pretty(&card).unwrap();
        let deserialized: Card = serde_json::from_str(&json).unwrap();
        assert_eq!(card, deserialized);
    }

    #[test]
    fn card_minimal_json_deserializes() {
        let json = r#"{
            "id": "01HXYZ1234567890ABCDEFGHIJ",
            "title": "Minimal",
            "column": "todo",
            "order": 0,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }"#;
        let card: Card = serde_json::from_str(json).unwrap();
        assert_eq!(card.title, "Minimal");
        assert!(card.labels.is_empty());
        assert!(!card.archived);
    }

    #[test]
    fn unique_ids() {
        let c1 = Card::new("A", "todo");
        let c2 = Card::new("B", "todo");
        assert_ne!(c1.id, c2.id);
    }
}
