use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(default)]
    pub commits: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_synced: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let meta = GitMetadata::default();
        assert!(meta.branch.is_none());
        assert!(meta.issue_url.is_none());
        assert!(meta.pr_url.is_none());
        assert!(meta.commits.is_empty());
        assert!(meta.last_synced.is_none());
    }

    #[test]
    fn roundtrip_json() {
        let meta = GitMetadata {
            branch: Some("feature/login".into()),
            issue_url: Some("https://github.com/user/repo/issues/42".into()),
            pr_url: None,
            commits: vec!["abc123".into(), "def456".into()],
            last_synced: Some(Utc::now()),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: GitMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.branch.unwrap(), "feature/login");
        assert_eq!(parsed.commits.len(), 2);
    }

    #[test]
    fn empty_json_deserializes() {
        let meta: GitMetadata = serde_json::from_str("{}").unwrap();
        assert!(meta.branch.is_none());
        assert!(meta.commits.is_empty());
    }

    #[test]
    fn skip_none_fields_in_serialization() {
        let meta = GitMetadata::default();
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("branch"));
        assert!(!json.contains("issue_url"));
        assert!(!json.contains("pr_url"));
        assert!(!json.contains("last_synced"));
    }
}
