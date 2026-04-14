use serde::Serialize;

/// Line range within a source file (1-based)
#[derive(Debug, Clone, Serialize)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

/// A single search result with scores and metadata
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    /// The chunk's text content
    pub chunk_text: String,
    /// Vault-relative file path
    pub file_path: String,
    /// Heading hierarchy breadcrumb
    pub heading_breadcrumb: String,
    /// Primary similarity score (0.0-1.0, higher = more relevant)
    /// In hybrid mode this is the normalized RRF score (or rank-based scores
    /// after Claude reranking when enabled).
    /// In single modes this copies the single-mode score.
    pub similarity_score: f64,
    /// Semantic (vector) score normalized to 0.0-1.0 (present in semantic/hybrid modes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_score: Option<f64>,
    /// Full-text search BM25 score normalized to 0.0-1.0 (present in fts/hybrid modes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fts_score: Option<f64>,
    /// Line range in the source file
    pub line_range: LineRange,
    /// Parsed frontmatter metadata
    pub frontmatter: serde_json::Value,
    /// Whether this result is a context chunk (not a direct match)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_context: Option<bool>,
    /// Index of the search result this context chunk belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_for_index: Option<usize>,
}

/// Wrapped search response per D-02
#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    /// The original query string
    pub query: String,
    /// Search mode used ("semantic", "fts", or "hybrid")
    pub mode: String,
    /// Total number of results returned
    pub total: usize,
    /// The search results
    pub results: Vec<SearchResult>,
}

/// Search mode selection (library-level, decoupled from CLI)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Vector similarity search using embeddings
    Semantic,
    /// Full-text search over chunk content (BM25)
    Fts,
    /// Hybrid search fusing semantic and FTS via RRF
    Hybrid,
}

impl std::fmt::Display for SearchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchMode::Semantic => write!(f, "semantic"),
            SearchMode::Fts => write!(f, "fts"),
            SearchMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

/// Internal options struct constructed from CLI args
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// The search query string
    pub query: String,
    /// Maximum number of results to return
    pub limit: usize,
    /// Minimum similarity score threshold (0.0-1.0)
    pub min_score: Option<f64>,
    /// Search mode
    pub mode: SearchMode,
    /// Filter results to files under this path prefix
    pub path_filter: Option<String>,
    /// Filter results to chunks with this frontmatter tag
    pub tag_filter: Option<String>,
    /// Number of surrounding context chunks to include per result
    pub context: usize,
    /// When true and a reranker is configured on `SearchEngine`, rerank retrieval
    /// results before applying `limit` and `min_score`.
    pub rerank: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_result_json_serialization() {
        let result = SearchResult {
            chunk_text: "Some chunk text".to_string(),
            file_path: "notes/test.md".to_string(),
            heading_breadcrumb: "# Title > ## Section".to_string(),
            similarity_score: 0.85,
            semantic_score: Some(0.9),
            fts_score: Some(0.7),
            line_range: LineRange { start: 1, end: 10 },
            frontmatter: serde_json::json!({"tags": ["rust", "search"]}),
            is_context: None,
            context_for_index: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("chunk_text").is_some());
        assert!(json.get("file_path").is_some());
        assert!(json.get("heading_breadcrumb").is_some());
        assert!(json.get("similarity_score").is_some());
        assert!(json.get("semantic_score").is_some());
        assert!(json.get("fts_score").is_some());
        assert!(json.get("line_range").is_some());
        assert!(json.get("frontmatter").is_some());

        let line_range = json.get("line_range").unwrap();
        assert_eq!(line_range.get("start").unwrap().as_u64().unwrap(), 1);
        assert_eq!(line_range.get("end").unwrap().as_u64().unwrap(), 10);
    }

    #[test]
    fn test_search_result_omits_null_scores() {
        let result = SearchResult {
            chunk_text: "text".to_string(),
            file_path: "file.md".to_string(),
            heading_breadcrumb: "# H".to_string(),
            similarity_score: 0.8,
            semantic_score: None,
            fts_score: Some(0.8),
            line_range: LineRange { start: 1, end: 5 },
            frontmatter: serde_json::json!({}),
            is_context: None,
            context_for_index: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(
            json.get("semantic_score").is_none(),
            "semantic_score should be omitted when None"
        );
        assert!(
            json.get("is_context").is_none(),
            "is_context should be omitted when None"
        );
        assert!(
            json.get("context_for_index").is_none(),
            "context_for_index should be omitted when None"
        );
        // fts_score should be present
        assert!(json.get("fts_score").is_some());
    }

    #[test]
    fn test_search_response_wrapped_object() {
        let response = SearchResponse {
            query: "test query".to_string(),
            mode: "hybrid".to_string(),
            total: 1,
            results: vec![SearchResult {
                chunk_text: "text".to_string(),
                file_path: "file.md".to_string(),
                heading_breadcrumb: "# H".to_string(),
                similarity_score: 0.5,
                semantic_score: Some(0.6),
                fts_score: Some(0.4),
                line_range: LineRange { start: 1, end: 2 },
                frontmatter: serde_json::json!({}),
                is_context: None,
                context_for_index: None,
            }],
        };

        let json = serde_json::to_value(&response).unwrap();
        assert!(json.get("query").is_some());
        assert!(json.get("mode").is_some());
        assert!(json.get("total").is_some());
        assert!(json.get("results").is_some());
        assert_eq!(json.get("query").unwrap().as_str().unwrap(), "test query");
        assert_eq!(json.get("mode").unwrap().as_str().unwrap(), "hybrid");
        assert_eq!(json.get("total").unwrap().as_u64().unwrap(), 1);
        assert_eq!(json.get("results").unwrap().as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_search_result_context_fields() {
        let result = SearchResult {
            chunk_text: "context chunk".to_string(),
            file_path: "file.md".to_string(),
            heading_breadcrumb: "# H".to_string(),
            similarity_score: 0.0,
            semantic_score: None,
            fts_score: None,
            line_range: LineRange { start: 5, end: 10 },
            frontmatter: serde_json::json!({}),
            is_context: Some(true),
            context_for_index: Some(0),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json.get("is_context").unwrap().as_bool().unwrap(), true);
        assert_eq!(json.get("context_for_index").unwrap().as_u64().unwrap(), 0);
    }
}
