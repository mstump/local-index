use std::sync::Arc;

use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::search::{SearchEngine, SearchMode, SearchOptions};
use crate::web::context::AppState;
use crate::web::error::AppError;

// -- Search --

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub mode: Option<String>,
    /// When set (e.g. `1` or `true`), skip reranking even if a reranker is configured.
    #[serde(default)]
    pub no_rerank: Option<bool>,
}

pub struct SearchResultView {
    pub file_path: String,
    pub heading_breadcrumb: String,
    pub chunk_text: String,
    pub similarity_score: f64,
}

#[derive(Template, WebTemplate)]
#[template(path = "search.html")]
pub struct SearchTemplate {
    pub query: Option<String>,
    pub mode: String,
    pub results: Vec<SearchResultView>,
    pub result_count: usize,
    pub active_nav: &'static str,
}

pub async fn search_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<SearchTemplate, AppError> {
    let mode_str = params.mode.unwrap_or_else(|| "hybrid".to_string());
    let query = params.q.clone().unwrap_or_default();

    // If no query, return empty form
    if query.is_empty() {
        return Ok(SearchTemplate {
            query: None,
            mode: mode_str,
            results: vec![],
            result_count: 0,
            active_nav: "search",
        });
    }

    let search_start = std::time::Instant::now();

    // Parse mode string to SearchMode enum
    let search_mode = match mode_str.as_str() {
        "semantic" => SearchMode::Semantic,
        "fts" => SearchMode::Fts,
        _ => SearchMode::Hybrid,
    };

    let skip_rerank = params.no_rerank.unwrap_or(false);
    let rerank = state.anthropic_reranker.is_some() && !skip_rerank;

    // Construct SearchEngine per-request (cheap -- just references + optional reranker clone)
    let engine = SearchEngine::new(&state.store, &*state.embedder)
        .with_anthropic_reranker(state.anthropic_reranker.clone());

    let opts = SearchOptions {
        query: query.clone(),
        limit: 20,
        min_score: None,
        mode: search_mode,
        path_filter: None,
        tag_filter: None,
        context: 0,
        rerank,
    };

    let response = engine.search(&opts).await?;

    let elapsed = search_start.elapsed();
    tracing::info!(
        query = %query,
        mode = %mode_str,
        results_returned = response.total,
        latency_ms = elapsed.as_millis() as u64,
        "web search completed"
    );

    let results: Vec<SearchResultView> = response
        .results
        .into_iter()
        .filter(|r| r.is_context != Some(true))
        .map(|r| {
            // Truncate chunk text to ~300 chars for preview
            let preview = if r.chunk_text.len() > 300 {
                let mut end = 300;
                // Don't cut in the middle of a multi-byte character
                while !r.chunk_text.is_char_boundary(end) && end < r.chunk_text.len() {
                    end += 1;
                }
                format!("{}...", &r.chunk_text[..end])
            } else {
                r.chunk_text.clone()
            };
            SearchResultView {
                file_path: r.file_path,
                heading_breadcrumb: r.heading_breadcrumb,
                chunk_text: preview,
                similarity_score: (r.similarity_score * 100.0).round() / 100.0,
            }
        })
        .collect();

    let result_count = results.len();

    Ok(SearchTemplate {
        query: Some(query),
        mode: mode_str,
        results,
        result_count,
        active_nav: "search",
    })
}

// -- Index Browser --

pub struct IndexFileView {
    pub file_path: String,
    pub chunk_count: usize,
    pub last_indexed: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub files: Vec<IndexFileView>,
    pub total_files: usize,
    pub total_chunks: usize,
    pub active_nav: &'static str,
}

pub async fn index_handler(
    State(state): State<Arc<AppState>>,
) -> Result<IndexTemplate, AppError> {
    let file_counts = state.store.count_chunks_per_file().await?;
    let total_chunks: usize = file_counts.iter().map(|(_, count)| count).sum();
    let total_files = file_counts.len();

    let files: Vec<IndexFileView> = file_counts
        .into_iter()
        .map(|(path, count)| IndexFileView {
            file_path: path,
            chunk_count: count,
            last_indexed: "\u{2014}".to_string(),
        })
        .collect();

    Ok(IndexTemplate {
        files,
        total_files,
        total_chunks,
        active_nav: "index",
    })
}

// -- Status --

#[derive(Template, WebTemplate)]
#[template(path = "status.html")]
pub struct StatusTemplate {
    pub total_files: usize,
    pub total_chunks: usize,
    pub last_index_time: String,
    pub queue_depth: usize,
    pub stale_files: usize,
    pub embedding_model: String,
    pub embedding_dimensions: usize,
    pub total_embeddings: usize,
    pub token_usage: String,
    pub active_nav: &'static str,
}

pub async fn status_handler(
    State(state): State<Arc<AppState>>,
) -> Result<StatusTemplate, AppError> {
    let total_chunks = state.store.count_total_chunks().await.unwrap_or(0);
    let total_files = state.store.count_distinct_files().await.unwrap_or(0);

    Ok(StatusTemplate {
        total_files,
        total_chunks,
        last_index_time: "\u{2014}".to_string(),
        queue_depth: 0,
        stale_files: 0,
        embedding_model: state.config.embedding_model.clone(),
        embedding_dimensions: state.config.embedding_dimensions,
        total_embeddings: total_chunks,
        token_usage: "N/A".to_string(),
        active_nav: "status",
    })
}

// -- Settings --

#[derive(Template, WebTemplate)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub data_dir: String,
    pub bind_addr: String,
    pub embedding_provider: String,
    pub credential_source: String,
    pub log_level: String,
    pub active_nav: &'static str,
}

pub async fn settings_handler(
    State(state): State<Arc<AppState>>,
) -> Result<SettingsTemplate, AppError> {
    let config = &state.config;
    Ok(SettingsTemplate {
        data_dir: config.data_dir.clone(),
        bind_addr: config.bind_addr.clone(),
        embedding_provider: config.embedding_provider.clone(),
        credential_source: config.credential_source.clone(),
        log_level: config.log_level.clone(),
        active_nav: "settings",
    })
}
