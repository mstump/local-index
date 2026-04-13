use std::sync::Arc;

use crate::claude_rerank::AnthropicReranker;
use crate::pipeline::embedder::VoyageEmbedder;
use crate::pipeline::store::ChunkStore;

/// Configuration values displayed in the dashboard.
/// Contains display-safe information only -- NEVER stores actual API keys.
#[derive(Debug, Clone)]
pub struct DashboardConfig {
    /// Display path for the data directory
    pub data_dir: String,
    /// HTTP bind address
    pub bind_addr: String,
    /// Current log level
    pub log_level: String,
    /// Description of where the credential comes from (e.g. "VOYAGE_API_KEY env var"),
    /// NEVER the actual key value (T-05-02 mitigation)
    pub credential_source: String,
    /// Embedding provider name (e.g. "voyage")
    pub embedding_provider: String,
    /// Embedding model name (e.g. "voyage-3.5")
    pub embedding_model: String,
    /// Embedding vector dimensions (e.g. 1024)
    pub embedding_dimensions: usize,
}

/// Shared application state passed to all dashboard handlers via axum State extractor.
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<ChunkStore>,
    pub embedder: Arc<VoyageEmbedder>,
    pub config: Arc<DashboardConfig>,
    pub anthropic_reranker: Option<AnthropicReranker>,
}
