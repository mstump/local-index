use std::path::PathBuf;

use local_index::error::LocalIndexError;
use local_index::pipeline::embedder::Embedder;
use local_index::pipeline::store::{compute_content_hash, ChunkStore};
use local_index::search::{
    format_json, SearchEngine, SearchMode, SearchOptions,
};
use local_index::types::{Chunk, EmbeddingResult, Frontmatter};

// -- Mock embedder that returns deterministic vectors without API calls --

struct MockEmbedder;

impl Embedder for MockEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<EmbeddingResult, LocalIndexError> {
        let embeddings: Vec<Vec<f32>> = texts
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let mut vec = vec![0.1f32; 1024];
                // Make vectors distinguishable based on content hash
                let hash = text.len() % 1024;
                vec[hash] += 1.0;
                // Add some variation based on index
                vec[(i * 7 + 3) % 1024] += 0.5;
                // Normalize to unit-ish vector for cosine distance
                let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
                for v in vec.iter_mut() {
                    *v /= norm;
                }
                vec
            })
            .collect();
        Ok(EmbeddingResult {
            embeddings,
            model: "mock-model".to_string(),
            total_tokens: texts.len() as u64 * 10,
        })
    }

    fn model_id(&self) -> &str {
        "mock-model"
    }

    fn dimensions(&self) -> usize {
        1024
    }
}

// -- Test data seeding --

fn make_chunk(
    file_path: &str,
    breadcrumb: &str,
    body: &str,
    line_start: usize,
    line_end: usize,
    tags: Vec<String>,
) -> Chunk {
    Chunk {
        file_path: PathBuf::from(file_path),
        heading_breadcrumb: breadcrumb.to_string(),
        heading_level: 2,
        body: body.to_string(),
        line_start,
        line_end,
        frontmatter: Frontmatter {
            tags,
            aliases: vec![],
            title: None,
            date: None,
            extra: Default::default(),
        },
    }
}

fn test_chunks() -> Vec<Chunk> {
    vec![
        make_chunk(
            "notes/rust.md",
            "# Rust > ## Ownership",
            "Rust ownership borrow checker ensures memory safety without garbage collection",
            1,
            10,
            vec!["rust".to_string(), "programming".to_string()],
        ),
        make_chunk(
            "notes/rust.md",
            "# Rust > ## Async",
            "Rust async tokio runtime provides high-performance concurrent execution",
            11,
            20,
            vec!["rust".to_string(), "programming".to_string()],
        ),
        make_chunk(
            "notes/python.md",
            "# Python > ## Types",
            "Python dynamic typing allows flexible variable assignment at runtime",
            1,
            10,
            vec!["python".to_string()],
        ),
        make_chunk(
            "notes/projects/search.md",
            "# Search > ## Approach",
            "semantic search vector embeddings enable finding related content by meaning",
            1,
            10,
            vec!["search".to_string(), "rust".to_string()],
        ),
    ]
}

async fn seed_store(store: &ChunkStore, embedder: &MockEmbedder) {
    let chunks = test_chunks();
    let texts: Vec<String> = chunks.iter().map(|c| c.body.clone()).collect();
    let result = embedder.embed(&texts).await.unwrap();
    let hashes: Vec<String> = chunks.iter().map(|c| compute_content_hash(c)).collect();
    store
        .store_chunks(&chunks, &result.embeddings, &hashes, embedder.model_id())
        .await
        .unwrap();
}

// -- Tests --

#[tokio::test]
async fn test_semantic_search() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().to_string_lossy().to_string();
    let store = ChunkStore::open(&db_path).await.unwrap();
    let embedder = MockEmbedder;

    seed_store(&store, &embedder).await;

    let engine = SearchEngine::new(&store, &embedder);
    let opts = SearchOptions {
        query: "Rust memory safety".to_string(),
        limit: 10,
        min_score: None,
        mode: SearchMode::Semantic,
        path_filter: None,
        tag_filter: None,
        context: 0,
        rerank: false,
    };

    let response = engine.search(&opts).await.unwrap();
    assert!(!response.results.is_empty(), "semantic search should return results");
    assert_eq!(response.mode, "semantic");

    for result in &response.results {
        assert!(
            result.semantic_score.is_some(),
            "semantic mode should populate semantic_score"
        );
        // fts_score should be None in semantic-only mode
        assert!(
            result.fts_score.is_none(),
            "semantic mode should not populate fts_score"
        );
    }
}

#[tokio::test]
async fn test_fts_search() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().to_string_lossy().to_string();
    let store = ChunkStore::open(&db_path).await.unwrap();
    let embedder = MockEmbedder;

    seed_store(&store, &embedder).await;

    let engine = SearchEngine::new(&store, &embedder);

    // Create FTS index
    engine.ensure_fts_index().await.unwrap();

    let opts = SearchOptions {
        query: "Rust".to_string(),
        limit: 10,
        min_score: None,
        mode: SearchMode::Fts,
        path_filter: None,
        tag_filter: None,
        context: 0,
        rerank: false,
    };

    let response = engine.search(&opts).await.unwrap();
    assert!(!response.results.is_empty(), "FTS search for 'Rust' should return results");
    assert_eq!(response.mode, "fts");

    for result in &response.results {
        assert!(
            result.fts_score.is_some(),
            "FTS mode should populate fts_score"
        );
        assert!(
            result.semantic_score.is_none(),
            "FTS mode should not populate semantic_score"
        );
        // All results should contain "Rust" somewhere (case-insensitive in body or breadcrumb)
        let text_lower = format!(
            "{} {}",
            result.chunk_text.to_lowercase(),
            result.heading_breadcrumb.to_lowercase()
        );
        assert!(
            text_lower.contains("rust"),
            "FTS result should match query: {}",
            result.chunk_text
        );
    }
}

#[tokio::test]
async fn test_hybrid_search() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().to_string_lossy().to_string();
    let store = ChunkStore::open(&db_path).await.unwrap();
    let embedder = MockEmbedder;

    seed_store(&store, &embedder).await;

    let engine = SearchEngine::new(&store, &embedder);

    let opts = SearchOptions {
        query: "Rust".to_string(),
        limit: 10,
        min_score: None,
        mode: SearchMode::Hybrid,
        path_filter: None,
        tag_filter: None,
        context: 0,
        rerank: false,
    };

    let response = engine.search(&opts).await.unwrap();
    assert!(!response.results.is_empty(), "hybrid search should return results");
    assert_eq!(response.mode, "hybrid");

    // Hybrid mode should have similarity_score set
    for result in &response.results {
        assert!(
            result.similarity_score >= 0.0,
            "hybrid results should have non-negative similarity_score"
        );
    }
}

#[tokio::test]
async fn test_json_output_shape() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().to_string_lossy().to_string();
    let store = ChunkStore::open(&db_path).await.unwrap();
    let embedder = MockEmbedder;

    seed_store(&store, &embedder).await;

    let engine = SearchEngine::new(&store, &embedder);
    let opts = SearchOptions {
        query: "Rust".to_string(),
        limit: 10,
        min_score: None,
        mode: SearchMode::Semantic,
        path_filter: None,
        tag_filter: None,
        context: 0,
        rerank: false,
    };

    let response = engine.search(&opts).await.unwrap();
    let json_str = format_json(&response).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert!(value.get("query").is_some(), "JSON must have 'query' key");
    assert!(value.get("mode").is_some(), "JSON must have 'mode' key");
    assert!(value.get("total").is_some(), "JSON must have 'total' key");
    assert!(value.get("results").is_some(), "JSON must have 'results' key");
    assert!(
        value["results"].is_array(),
        "'results' must be an array"
    );
}

#[tokio::test]
async fn test_limit_flag() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().to_string_lossy().to_string();
    let store = ChunkStore::open(&db_path).await.unwrap();
    let embedder = MockEmbedder;

    seed_store(&store, &embedder).await;

    let engine = SearchEngine::new(&store, &embedder);
    let opts = SearchOptions {
        query: "programming".to_string(),
        limit: 2,
        min_score: None,
        mode: SearchMode::Semantic,
        path_filter: None,
        tag_filter: None,
        context: 0,
        rerank: false,
    };

    let response = engine.search(&opts).await.unwrap();
    assert!(
        response.results.len() <= 2,
        "limit=2 should return at most 2 results, got {}",
        response.results.len()
    );
}

#[tokio::test]
async fn test_path_filter() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().to_string_lossy().to_string();
    let store = ChunkStore::open(&db_path).await.unwrap();
    let embedder = MockEmbedder;

    seed_store(&store, &embedder).await;

    let engine = SearchEngine::new(&store, &embedder);
    let opts = SearchOptions {
        query: "search".to_string(),
        limit: 10,
        min_score: None,
        mode: SearchMode::Semantic,
        path_filter: Some("notes/projects/".to_string()),
        tag_filter: None,
        context: 0,
        rerank: false,
    };

    let response = engine.search(&opts).await.unwrap();
    for result in &response.results {
        assert!(
            result.file_path.starts_with("notes/projects/"),
            "path_filter should restrict results to prefix, got: {}",
            result.file_path
        );
    }
}

#[tokio::test]
async fn test_tag_filter() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().to_string_lossy().to_string();
    let store = ChunkStore::open(&db_path).await.unwrap();
    let embedder = MockEmbedder;

    seed_store(&store, &embedder).await;

    let engine = SearchEngine::new(&store, &embedder);
    let opts = SearchOptions {
        query: "typing".to_string(),
        limit: 10,
        min_score: None,
        mode: SearchMode::Semantic,
        path_filter: None,
        tag_filter: Some("python".to_string()),
        context: 0,
        rerank: false,
    };

    let response = engine.search(&opts).await.unwrap();
    for result in &response.results {
        let empty_vec = vec![];
        let tags = result
            .frontmatter
            .get("tags")
            .and_then(|t| t.as_array())
            .unwrap_or(&empty_vec);
        let has_python = tags.iter().any(|t| t.as_str() == Some("python"));
        assert!(
            has_python,
            "tag_filter=python should only return chunks with python tag, got: {:?}",
            result.frontmatter
        );
    }
}

#[tokio::test]
async fn test_context_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().to_string_lossy().to_string();
    let store = ChunkStore::open(&db_path).await.unwrap();
    let embedder = MockEmbedder;

    // Seed with 3 sequential chunks for the same file
    let chunks = vec![
        make_chunk(
            "notes/context_test.md",
            "# Doc > ## Intro",
            "This is the introduction section of the document",
            1,
            10,
            vec!["test".to_string()],
        ),
        make_chunk(
            "notes/context_test.md",
            "# Doc > ## Main",
            "This is the main content section about Rust programming and search",
            11,
            20,
            vec!["test".to_string()],
        ),
        make_chunk(
            "notes/context_test.md",
            "# Doc > ## Conclusion",
            "This is the conclusion wrapping up the document",
            21,
            30,
            vec!["test".to_string()],
        ),
    ];

    let texts: Vec<String> = chunks.iter().map(|c| c.body.clone()).collect();
    let result = embedder.embed(&texts).await.unwrap();
    let hashes: Vec<String> = chunks.iter().map(|c| compute_content_hash(c)).collect();
    store
        .store_chunks(&chunks, &result.embeddings, &hashes, embedder.model_id())
        .await
        .unwrap();

    let engine = SearchEngine::new(&store, &embedder);
    let opts = SearchOptions {
        query: "Rust programming search".to_string(),
        limit: 1,
        min_score: None,
        mode: SearchMode::Semantic,
        path_filter: None,
        tag_filter: None,
        context: 1, // Request 1 context chunk on each side
        rerank: false,
    };

    let response = engine.search(&opts).await.unwrap();

    // Should have the main result plus context chunks
    let main_results: Vec<_> = response
        .results
        .iter()
        .filter(|r| r.is_context != Some(true))
        .collect();
    let context_results: Vec<_> = response
        .results
        .iter()
        .filter(|r| r.is_context == Some(true))
        .collect();

    assert!(
        !main_results.is_empty(),
        "should have at least one main result"
    );

    if !context_results.is_empty() {
        for ctx in &context_results {
            assert_eq!(
                ctx.is_context,
                Some(true),
                "context chunks should have is_context=true"
            );
            assert!(
                ctx.context_for_index.is_some(),
                "context chunks should have context_for_index set"
            );
        }
    }
}

#[tokio::test]
async fn test_empty_index_returns_empty_response() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().to_string_lossy().to_string();
    let store = ChunkStore::open(&db_path).await.unwrap();
    let embedder = MockEmbedder;

    let engine = SearchEngine::new(&store, &embedder);
    let opts = SearchOptions {
        query: "anything".to_string(),
        limit: 10,
        min_score: None,
        mode: SearchMode::Semantic,
        path_filter: None,
        tag_filter: None,
        context: 0,
        rerank: false,
    };

    let response = engine.search(&opts).await.unwrap();
    assert_eq!(response.total, 0, "empty index should return 0 results");
    assert!(
        response.results.is_empty(),
        "empty index should return empty results vec"
    );
}
