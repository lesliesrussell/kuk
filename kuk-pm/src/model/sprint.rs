use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SprintStatus {
    Planned,
    Active,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sprint {
    pub name: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(default)]
    pub boards: Vec<String>,
    pub status: SprintStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprint_roundtrip() {
        let sprint = Sprint {
            name: "Q1-2026".into(),
            start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
            goal: Some("Ship MVP".into()),
            boards: vec!["default".into(), "sprint-1".into()],
            status: SprintStatus::Active,
        };
        let json = serde_json::to_string_pretty(&sprint).unwrap();
        let parsed: Sprint = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Q1-2026");
        assert_eq!(parsed.status, SprintStatus::Active);
        assert_eq!(parsed.boards.len(), 2);
    }

    #[test]
    fn sprint_status_serializes_lowercase() {
        let json = serde_json::to_string(&SprintStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");
    }

    #[test]
    fn sprint_minimal_json() {
        let json = r#"{
            "name": "test",
            "start": "2026-01-01",
            "end": "2026-01-31",
            "status": "planned"
        }"#;
        let sprint: Sprint = serde_json::from_str(json).unwrap();
        assert_eq!(sprint.name, "test");
        assert!(sprint.goal.is_none());
        assert!(sprint.boards.is_empty());
    }

    #[test]
    fn sprint_date_range() {
        let sprint = Sprint {
            name: "week-1".into(),
            start: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 2, 7).unwrap(),
            goal: None,
            boards: Vec::new(),
            status: SprintStatus::Planned,
        };
        let duration = sprint.end - sprint.start;
        assert_eq!(duration.num_days(), 6);
    }
}
