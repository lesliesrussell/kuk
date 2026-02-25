use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{KukError, Result};
use crate::model::{Board, GlobalIndex, RepoConfig};

/// The core storage layer. All file I/O goes through here.
pub struct Store {
    repo_root: PathBuf,
}

impl Store {
    /// Create a Store rooted at the given directory.
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    /// The .kuk directory path.
    pub fn kuk_dir(&self) -> PathBuf {
        self.repo_root.join(".kuk")
    }

    fn boards_dir(&self) -> PathBuf {
        self.kuk_dir().join("boards")
    }

    fn config_path(&self) -> PathBuf {
        self.kuk_dir().join("config.json")
    }

    fn board_path(&self, name: &str) -> PathBuf {
        self.boards_dir().join(format!("{name}.json"))
    }

    /// Check if .kuk/ exists.
    pub fn is_initialized(&self) -> bool {
        self.kuk_dir().exists()
    }

    /// Initialize .kuk/ with default config and board.
    pub fn init(&self) -> Result<()> {
        if self.is_initialized() {
            return Err(KukError::AlreadyInitialized(
                self.kuk_dir().display().to_string(),
            ));
        }

        fs::create_dir_all(self.boards_dir())?;

        let config = RepoConfig::default();
        self.write_json(&self.config_path(), &config)?;

        let board = Board::default_board();
        self.write_json(&self.board_path(&board.name), &board)?;

        // Register in global index
        if let Some(global) = Self::global_index_path() {
            let mut index = Self::load_global_index().unwrap_or_default();
            let name = self
                .repo_root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into());
            index.add(self.repo_root.display().to_string(), name);
            if let Some(parent) = global.parent() {
                fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_string_pretty(&index)?;
            fs::write(&global, json)?;
        }

        Ok(())
    }

    /// Load per-repo config.
    pub fn load_config(&self) -> Result<RepoConfig> {
        self.ensure_initialized()?;
        let data = fs::read_to_string(self.config_path())?;
        Ok(serde_json::from_str(&data)?)
    }

    /// Save per-repo config.
    pub fn save_config(&self, config: &RepoConfig) -> Result<()> {
        self.ensure_initialized()?;
        self.write_json(&self.config_path(), config)
    }

    /// Load a board by name.
    pub fn load_board(&self, name: &str) -> Result<Board> {
        self.ensure_initialized()?;
        let path = self.board_path(name);
        if !path.exists() {
            return Err(KukError::BoardNotFound(name.into()));
        }
        let data = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&data)?)
    }

    /// Save a board.
    pub fn save_board(&self, board: &Board) -> Result<()> {
        self.ensure_initialized()?;
        self.write_json(&self.board_path(&board.name), board)
    }

    /// List all board names.
    pub fn list_boards(&self) -> Result<Vec<String>> {
        self.ensure_initialized()?;
        let mut boards = Vec::new();
        for entry in fs::read_dir(self.boards_dir())? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Some(stem) = path.file_stem()
            {
                boards.push(stem.to_string_lossy().to_string());
            }
        }
        boards.sort();
        Ok(boards)
    }

    /// Create a new board.
    pub fn create_board(&self, name: &str, columns: Vec<crate::model::Column>) -> Result<()> {
        self.ensure_initialized()?;
        let path = self.board_path(name);
        if path.exists() {
            return Err(KukError::Other(format!("Board already exists: {name}")));
        }
        let board = Board {
            name: name.into(),
            columns,
            cards: Vec::new(),
        };
        self.write_json(&path, &board)
    }

    // --- Global index ---

    fn global_index_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".kuk").join("index.json"))
    }

    pub fn load_global_index() -> Option<GlobalIndex> {
        let path = Self::global_index_path()?;
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    // --- Helpers ---

    fn ensure_initialized(&self) -> Result<()> {
        if !self.is_initialized() {
            Err(KukError::NotInitialized)
        } else {
            Ok(())
        }
    }

    fn write_json<T: serde::Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        let json = serde_json::to_string_pretty(value)?;
        fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::new(dir.path());
        (dir, store)
    }

    #[test]
    fn not_initialized_before_init() {
        let (_dir, store) = temp_store();
        assert!(!store.is_initialized());
    }

    #[test]
    fn init_creates_structure() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        assert!(store.is_initialized());
        assert!(store.kuk_dir().join("config.json").exists());
        assert!(store.kuk_dir().join("boards/default.json").exists());
    }

    #[test]
    fn init_idempotent_returns_error() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        let result = store.init();
        assert!(result.is_err());
        match result.unwrap_err() {
            KukError::AlreadyInitialized(_) => {}
            other => panic!("Expected AlreadyInitialized, got: {other}"),
        }
    }

    #[test]
    fn load_config_after_init() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        let config = store.load_config().unwrap();
        assert_eq!(config.version, "0.1.0");
        assert_eq!(config.default_board, "default");
    }

    #[test]
    fn load_config_before_init_fails() {
        let (_dir, store) = temp_store();
        assert!(store.load_config().is_err());
    }

    #[test]
    fn save_config_persists() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        let mut config = store.load_config().unwrap();
        assert_eq!(config.default_board, "default");
        config.default_board = "sprint-1".into();
        store.save_config(&config).unwrap();
        let reloaded = store.load_config().unwrap();
        assert_eq!(reloaded.default_board, "sprint-1");
    }

    #[test]
    fn load_default_board() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        let board = store.load_board("default").unwrap();
        assert_eq!(board.name, "default");
        assert_eq!(board.columns.len(), 3);
        assert!(board.cards.is_empty());
    }

    #[test]
    fn load_nonexistent_board_fails() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        assert!(store.load_board("nonexistent").is_err());
    }

    #[test]
    fn save_and_reload_board() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        let mut board = store.load_board("default").unwrap();
        board.cards.push(crate::model::Card::new("Task 1", "todo"));
        store.save_board(&board).unwrap();

        let reloaded = store.load_board("default").unwrap();
        assert_eq!(reloaded.cards.len(), 1);
        assert_eq!(reloaded.cards[0].title, "Task 1");
    }

    #[test]
    fn list_boards() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        let boards = store.list_boards().unwrap();
        assert_eq!(boards, vec!["default"]);
    }

    #[test]
    fn create_board() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        store
            .create_board(
                "sprint-1",
                vec![
                    crate::model::Column {
                        name: "backlog".into(),
                        wip_limit: None,
                    },
                    crate::model::Column {
                        name: "active".into(),
                        wip_limit: Some(3),
                    },
                ],
            )
            .unwrap();
        let boards = store.list_boards().unwrap();
        assert!(boards.contains(&"sprint-1".to_string()));
        let board = store.load_board("sprint-1").unwrap();
        assert_eq!(board.columns.len(), 2);
        assert_eq!(board.columns[1].wip_limit, Some(3));
    }

    #[test]
    fn create_duplicate_board_fails() {
        let (_dir, store) = temp_store();
        store.init().unwrap();
        let result = store.create_board(
            "default",
            vec![crate::model::Column {
                name: "col".into(),
                wip_limit: None,
            }],
        );
        assert!(result.is_err());
    }

    #[test]
    fn save_board_before_init_fails() {
        let (_dir, store) = temp_store();
        let board = Board::default_board();
        assert!(store.save_board(&board).is_err());
    }
}
