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
    #[error("Rerank error: {0}")]
    Rerank(String),
    #[error("Ignore walk error: {0}")]
    Ignore(#[from] ignore::Error),
    #[error("asset too large: {bytes} bytes (max {max_bytes})")]
    AssetTooLarge { bytes: usize, max_bytes: usize },
    #[error("PDF error: {0}")]
    Pdf(#[from] lopdf::Error),
}

impl LocalIndexError {
    /// Returns true if the error is transient and the operation should be retried.
    /// Applies to Embedding errors caused by rate limiting (429) or server errors (5xx).
    pub fn is_transient(&self) -> bool {
        match self {
            LocalIndexError::Embedding(msg) | LocalIndexError::Rerank(msg) => {
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
