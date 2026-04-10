#[derive(Debug, thiserror::Error)]
pub enum LocalIndexError {
    #[error("Chunk error: {0}")]
    Chunk(String),
    #[error("Walk error: {0}")]
    Walk(#[from] walkdir::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML parse error: {0}")]
    YamlParse(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("Credential error: {0}")]
    Credential(String),
    #[error("Embedding error: {0}")]
    Embedding(String),
    #[error("Database error: {0}")]
    Database(String),
}

impl LocalIndexError {
    /// Returns true if the error is transient and the operation should be retried.
    /// Applies to Embedding errors caused by rate limiting (429) or server errors (5xx).
    pub fn is_transient(&self) -> bool {
        match self {
            LocalIndexError::Embedding(msg) => {
                let lower = msg.to_lowercase();
                lower.contains("429")
                    || lower.contains("500")
                    || lower.contains("502")
                    || lower.contains("503")
                    || lower.contains("504")
                    || lower.contains("timeout")
            }
            _ => false,
        }
    }
}
