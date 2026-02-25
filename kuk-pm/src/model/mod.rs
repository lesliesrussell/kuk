mod git_meta;
mod project;
mod sprint;

pub use git_meta::GitMetadata;
pub use project::PmProject;
pub use sprint::{Sprint, SprintStatus};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmConfig {
    pub version: String,
    pub auto_branch: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_provider: Option<String>,
}

impl Default for PmConfig {
    fn default() -> Self {
        Self {
            version: "0.1.0".into(),
            auto_branch: false,
            sync_provider: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pm_config_default() {
        let config = PmConfig::default();
        assert_eq!(config.version, "0.1.0");
        assert!(!config.auto_branch);
        assert!(config.sync_provider.is_none());
    }

    #[test]
    fn pm_config_roundtrip() {
        let config = PmConfig {
            version: "0.1.0".into(),
            auto_branch: true,
            sync_provider: Some("github".into()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: PmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, "0.1.0");
        assert!(parsed.auto_branch);
        assert_eq!(parsed.sync_provider.unwrap(), "github");
    }

    #[test]
    fn pm_config_skip_none_sync() {
        let config = PmConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("sync_provider"));
    }
}
