use thiserror::Error;

#[derive(Error, Debug)]
pub enum PmError {
    #[error("Not initialized. Run `kuk init` then `kuk-pm init` first.")]
    NotInitialized,

    #[error("kuk not initialized. Run `kuk init` first.")]
    KukNotInitialized,

    #[error("Already initialized at {0}")]
    AlreadyInitialized(String),

    #[error("Not a git repository")]
    NotGitRepo,

    #[error("Git error: {0}")]
    Git(String),

    #[error("Card not found: {0}")]
    CardNotFound(String),

    #[error("Sprint not found: {0}")]
    SprintNotFound(String),

    #[error("Sprint already exists: {0}")]
    SprintAlreadyExists(String),

    #[error("Sprint already closed: {0}")]
    SprintAlreadyClosed(String),

    #[error("No active sprint found")]
    NoActiveSprint,

    #[error("Invalid date: {0}")]
    InvalidDate(String),

    #[error("Not yet implemented: {0}")]
    NotImplemented(String),

    #[error(transparent)]
    Kuk(#[from] kuk::error::KukError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, PmError>;
