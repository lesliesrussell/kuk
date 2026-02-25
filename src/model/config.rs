use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepoConfig {
    pub version: String,
    #[serde(default = "default_board")]
    pub default_board: String,
}

fn default_board() -> String {
    "default".into()
}

impl Default for RepoConfig {
    fn default() -> Self {
        Self {
            version: "0.1.0".into(),
            default_board: "default".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = RepoConfig::default();
        assert_eq!(config.version, "0.1.0");
        assert_eq!(config.default_board, "default");
    }

    #[test]
    fn config_roundtrip() {
        let config = RepoConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: RepoConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn config_missing_default_board_uses_default() {
        let json = r#"{"version": "0.1.0"}"#;
        let config: RepoConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.default_board, "default");
    }
}
