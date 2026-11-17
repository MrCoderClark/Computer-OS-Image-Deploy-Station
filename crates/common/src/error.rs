use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImagingError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("disk error: {0}")]
    Disk(String),

    #[error("image error: {0}")]
    Image(String),

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("operation cancelled")]
    Cancelled,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
