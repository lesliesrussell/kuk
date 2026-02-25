use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexEntry {
    pub path: String,
    pub name: String,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GlobalIndex {
    pub projects: Vec<IndexEntry>,
}

impl GlobalIndex {
    pub fn add(&mut self, path: impl Into<String>, name: impl Into<String>) {
        let path = path.into();
        if !self.projects.iter().any(|p| p.path == path) {
            self.projects.push(IndexEntry {
                path,
                name: name.into(),
                added_at: Utc::now(),
            });
        }
    }

    pub fn remove(&mut self, path: &str) {
        self.projects.retain(|p| p.path != path);
    }

    pub fn contains(&self, path: &str) -> bool {
        self.projects.iter().any(|p| p.path == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_index() {
        let index = GlobalIndex::default();
        assert!(index.projects.is_empty());
    }

    #[test]
    fn add_project() {
        let mut index = GlobalIndex::default();
        index.add("/home/user/project", "project");
        assert_eq!(index.projects.len(), 1);
        assert!(index.contains("/home/user/project"));
    }

    #[test]
    fn add_duplicate_is_noop() {
        let mut index = GlobalIndex::default();
        index.add("/home/user/project", "project");
        index.add("/home/user/project", "project");
        assert_eq!(index.projects.len(), 1);
    }

    #[test]
    fn remove_project() {
        let mut index = GlobalIndex::default();
        index.add("/home/user/project", "project");
        index.remove("/home/user/project");
        assert!(index.projects.is_empty());
    }

    #[test]
    fn remove_nonexistent_is_noop() {
        let mut index = GlobalIndex::default();
        index.remove("/nonexistent");
        assert!(index.projects.is_empty());
    }

    #[test]
    fn index_roundtrip() {
        let mut index = GlobalIndex::default();
        index.add("/a", "a");
        index.add("/b", "b");
        let json = serde_json::to_string_pretty(&index).unwrap();
        let deserialized: GlobalIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(index, deserialized);
    }
}
