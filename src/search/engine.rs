use crate::error::LocalIndexError;
use crate::pipeline::embedder::Embedder;
use crate::pipeline::store::ChunkStore;

use super::types::*;

/// Search engine that wraps ChunkStore + Embedder and dispatches queries
/// through LanceDB's vector, FTS, and hybrid search APIs.
pub struct SearchEngine<'a, E: Embedder> {
    #[allow(dead_code)]
    store: &'a ChunkStore,
    #[allow(dead_code)]
    embedder: &'a E,
}

impl<'a, E: Embedder> SearchEngine<'a, E> {
    /// Create a new SearchEngine with references to the store and embedder.
    pub fn new(store: &'a ChunkStore, embedder: &'a E) -> Self {
        Self { store, embedder }
    }

    /// Execute a search query, dispatching to the appropriate search mode.
    pub async fn search(&self, _opts: &SearchOptions) -> Result<SearchResponse, LocalIndexError> {
        // Placeholder -- will be implemented in Task 2
        todo!("SearchEngine::search not yet implemented")
    }
}
