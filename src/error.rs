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
}
