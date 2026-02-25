use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmProject {
    pub name: String,
    pub repos: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_roundtrip() {
        let project = PmProject {
            name: "my-app".into(),
            repos: vec!["/home/user/frontend".into(), "/home/user/backend".into()],
            description: Some("Full stack app".into()),
        };
        let json = serde_json::to_string(&project).unwrap();
        let parsed: PmProject = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "my-app");
        assert_eq!(parsed.repos.len(), 2);
    }

    #[test]
    fn project_minimal() {
        let json = r#"{"name": "test", "repos": []}"#;
        let project: PmProject = serde_json::from_str(json).unwrap();
        assert!(project.description.is_none());
        assert!(project.repos.is_empty());
    }
}
