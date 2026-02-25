use thiserror::Error;

#[derive(Debug, Error)]
pub enum KukError {
    #[error("Not a kuk project. Run `kuk init` first.")]
    NotInitialized,

    #[error("Already initialized at {0}")]
    AlreadyInitialized(String),

    #[error("Board not found: {0}")]
    BoardNotFound(String),

    #[error("Card not found: {0}")]
    CardNotFound(String),

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("Label not found on card: {0}")]
    LabelNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, KukError>;
