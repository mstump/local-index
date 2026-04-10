use std::sync::Arc;

use arrow_array::{Array, RecordBatch, StringArray, UInt32Array};
use arrow_array::types::{Float32Type, Float64Type};
use arrow_array::cast::AsArray;
use futures::TryStreamExt;
use lance_index::scalar::FullTextSearchQuery;
use lancedb::index::{scalar::FtsIndexBuilder, Index};
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::rerankers::rrf::RRFReranker;
use lancedb::DistanceType;

use crate::error::LocalIndexError;
use crate::pipeline::embedder::Embedder;
use crate::pipeline::store::ChunkStore;

use super::types::*;

/// Over-fetch multiplier when tag filtering is active.
/// We fetch limit * TAG_OVERFETCH_MULTIPLIER results, then post-filter in Rust.
const TAG_OVERFETCH_MULTIPLIER: usize = 3;

/// Search engine that wraps ChunkStore + Embedder and dispatches queries
/// through LanceDB's vector, FTS, and hybrid search APIs.
pub struct SearchEngine<'a, E: Embedder> {
    store: &'a ChunkStore,
    embedder: &'a E,
}

impl<'a, E: Embedder> SearchEngine<'a, E> {
    /// Create a new SearchEngine with references to the store and embedder.
    pub fn new(store: &'a ChunkStore, embedder: &'a E) -> Self {
        Self { store, embedder }
    }

    /// Execute a search query, dispatching to the appropriate search mode.
    ///
    /// After getting results, applies min_score filter and context chunk fetching.
    pub async fn search(&self, opts: &SearchOptions) -> Result<SearchResponse, LocalIndexError> {
        let mut results = match opts.mode {
            SearchMode::Semantic => self.semantic_search(opts).await?,
            SearchMode::Fts => self.fts_search(opts).await?,
            SearchMode::Hybrid => self.hybrid_search(opts).await?,
        };

        // Apply min_score filter
        if let Some(min_score) = opts.min_score {
            results.retain(|r| r.similarity_score >= min_score);
        }

        // Fetch context chunks if requested
        if opts.context > 0 && !results.is_empty() {
            self.fetch_context_chunks(&mut results, opts.context).await?;
        }

        Ok(SearchResponse {
            query: opts.query.clone(),
            mode: opts.mode.to_string(),
            total: results.len(),
            results,
        })
    }

    /// Ensure the FTS index exists on the body column.
    /// LanceDB's create_index rebuilds the index each time (idempotent).
    pub async fn ensure_fts_index(&self) -> Result<(), LocalIndexError> {
        self.store
            .table()
            .create_index(&["body"], Index::FTS(FtsIndexBuilder::default()))
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(format!("FTS index creation failed: {}", e)))?;
        Ok(())
    }

    /// Semantic (vector ANN) search using embeddings.
    async fn semantic_search(
        &self,
        opts: &SearchOptions,
    ) -> Result<Vec<SearchResult>, LocalIndexError> {
        // Embed the query
        let embedding_result = self.embedder.embed(&[opts.query.clone()]).await?;
        let query_vec = embedding_result
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| {
                LocalIndexError::Embedding("No embedding returned for query".to_string())
            })?;

        let effective_limit = if opts.tag_filter.is_some() {
            opts.limit * TAG_OVERFETCH_MULTIPLIER
        } else {
            opts.limit
        };

        // Build vector query
        let mut query = self
            .store
            .table()
            .query()
            .nearest_to(query_vec.as_slice())
            .map_err(|e| LocalIndexError::Database(e.to_string()))?
            .distance_type(DistanceType::Cosine)
            .limit(effective_limit);

        // Apply path filter
        if let Some(ref path_filter) = opts.path_filter {
            let escaped = path_filter.replace('\'', "''");
            query = query.only_if(format!("file_path LIKE '{}%'", escaped));
        }

        // Execute
        let batches: Vec<RecordBatch> = query
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        // Extract results with semantic scoring
        let mut results = extract_results_from_batches(&batches, ScoreMode::Semantic);

        // Post-filter by tag if needed
        if let Some(ref tag) = opts.tag_filter {
            results.retain(|r| frontmatter_has_tag(&r.frontmatter, tag));
        }

        // Truncate to limit
        results.truncate(opts.limit);

        Ok(results)
    }

    /// Full-text search using BM25 scoring.
    async fn fts_search(
        &self,
        opts: &SearchOptions,
    ) -> Result<Vec<SearchResult>, LocalIndexError> {
        // Ensure FTS index exists
        self.ensure_fts_index().await?;

        let effective_limit = if opts.tag_filter.is_some() {
            opts.limit * TAG_OVERFETCH_MULTIPLIER
        } else {
            opts.limit
        };

        // Build FTS query
        let mut query = self
            .store
            .table()
            .query()
            .full_text_search(FullTextSearchQuery::new(opts.query.clone()))
            .limit(effective_limit);

        // Apply path filter
        if let Some(ref path_filter) = opts.path_filter {
            let escaped = path_filter.replace('\'', "''");
            query = query.only_if(format!("file_path LIKE '{}%'", escaped));
        }

        // Execute
        let batches: Vec<RecordBatch> = query
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        // Extract results with FTS scoring
        let mut results = extract_results_from_batches(&batches, ScoreMode::Fts);

        // Post-filter by tag if needed
        if let Some(ref tag) = opts.tag_filter {
            results.retain(|r| frontmatter_has_tag(&r.frontmatter, tag));
        }

        // Truncate to limit
        results.truncate(opts.limit);

        Ok(results)
    }

    /// Hybrid search using RRF fusion of semantic and FTS.
    async fn hybrid_search(
        &self,
        opts: &SearchOptions,
    ) -> Result<Vec<SearchResult>, LocalIndexError> {
        // Ensure FTS index exists
        self.ensure_fts_index().await?;

        // Embed the query for the vector component
        let embedding_result = self.embedder.embed(&[opts.query.clone()]).await?;
        let query_vec = embedding_result
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| {
                LocalIndexError::Embedding("No embedding returned for query".to_string())
            })?;

        let effective_limit = if opts.tag_filter.is_some() {
            opts.limit * TAG_OVERFETCH_MULTIPLIER
        } else {
            opts.limit
        };

        // Build hybrid query: FTS + vector + RRF reranker
        let mut query = self
            .store
            .table()
            .query()
            .full_text_search(FullTextSearchQuery::new(opts.query.clone()))
            .nearest_to(query_vec.as_slice())
            .map_err(|e| LocalIndexError::Database(e.to_string()))?
            .distance_type(DistanceType::Cosine)
            .rerank(Arc::new(RRFReranker::new(60.0)))
            .limit(effective_limit);

        // Apply path filter
        if let Some(ref path_filter) = opts.path_filter {
            let escaped = path_filter.replace('\'', "''");
            query = query.only_if(format!("file_path LIKE '{}%'", escaped));
        }

        // Execute
        let batches: Vec<RecordBatch> = query
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        // Extract results with hybrid scoring
        let mut results = extract_results_from_batches(&batches, ScoreMode::Hybrid);

        // Post-filter by tag if needed
        if let Some(ref tag) = opts.tag_filter {
            results.retain(|r| frontmatter_has_tag(&r.frontmatter, tag));
        }

        // Truncate to limit
        results.truncate(opts.limit);

        Ok(results)
    }

    /// Fetch context chunks (adjacent chunks from same file) for each search result.
    async fn fetch_context_chunks(
        &self,
        results: &mut Vec<SearchResult>,
        context_n: usize,
    ) -> Result<(), LocalIndexError> {
        let original_count = results.len();
        let mut context_chunks: Vec<SearchResult> = Vec::new();

        for (idx, result) in results.iter().enumerate().take(original_count) {
            let escaped_path = result.file_path.replace('\'', "''");

            // Query all chunks from the same file
            let batches: Vec<RecordBatch> = self
                .store
                .table()
                .query()
                .only_if(format!("file_path = '{}'", escaped_path))
                .select(Select::Columns(vec![
                    "file_path".to_string(),
                    "heading_breadcrumb".to_string(),
                    "body".to_string(),
                    "line_start".to_string(),
                    "line_end".to_string(),
                    "frontmatter_json".to_string(),
                ]))
                .execute()
                .await
                .map_err(|e| LocalIndexError::Database(e.to_string()))?
                .try_collect()
                .await
                .map_err(|e| LocalIndexError::Database(e.to_string()))?;

            // Extract all chunks from this file
            let mut file_chunks: Vec<(u32, u32, String, String, String, serde_json::Value)> =
                Vec::new();
            for batch in &batches {
                let n = batch.num_rows();
                let line_starts = get_u32_column(batch, "line_start");
                let line_ends = get_u32_column(batch, "line_end");
                let bodies = get_string_column(batch, "body");
                let breadcrumbs = get_string_column(batch, "heading_breadcrumb");
                let file_paths = get_string_column(batch, "file_path");
                let frontmatters = get_string_column(batch, "frontmatter_json");

                for i in 0..n {
                    let fm_json = frontmatters
                        .as_ref()
                        .and_then(|arr| {
                            if arr.is_null(i) {
                                None
                            } else {
                                serde_json::from_str(arr.value(i)).ok()
                            }
                        })
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                    file_chunks.push((
                        line_starts.as_ref().map(|a| a.value(i)).unwrap_or(0),
                        line_ends.as_ref().map(|a| a.value(i)).unwrap_or(0),
                        bodies
                            .as_ref()
                            .map(|a| a.value(i).to_string())
                            .unwrap_or_default(),
                        breadcrumbs
                            .as_ref()
                            .map(|a| a.value(i).to_string())
                            .unwrap_or_default(),
                        file_paths
                            .as_ref()
                            .map(|a| a.value(i).to_string())
                            .unwrap_or_default(),
                        fm_json,
                    ));
                }
            }

            // Sort by line_start
            file_chunks.sort_by_key(|c| c.0);

            // Find the matching chunk by line range
            let match_idx = file_chunks
                .iter()
                .position(|c| c.0 == result.line_range.start && c.1 == result.line_range.end);

            if let Some(mi) = match_idx {
                // Take N before and N after
                let start = mi.saturating_sub(context_n);
                let end = (mi + context_n + 1).min(file_chunks.len());

                for ci in start..end {
                    if ci == mi {
                        continue; // Skip the match itself
                    }
                    let chunk = &file_chunks[ci];
                    context_chunks.push(SearchResult {
                        chunk_text: chunk.2.clone(),
                        file_path: chunk.4.clone(),
                        heading_breadcrumb: chunk.3.clone(),
                        similarity_score: 0.0,
                        semantic_score: None,
                        fts_score: None,
                        line_range: LineRange {
                            start: chunk.0,
                            end: chunk.1,
                        },
                        frontmatter: chunk.5.clone(),
                        is_context: Some(true),
                        context_for_index: Some(idx),
                    });
                }
            }
        }

        results.extend(context_chunks);
        Ok(())
    }
}

// -- Score extraction modes --

#[derive(Debug, Clone, Copy)]
enum ScoreMode {
    Semantic,
    Fts,
    Hybrid,
}

/// Extract SearchResults from Arrow RecordBatches with score normalization.
fn extract_results_from_batches(batches: &[RecordBatch], mode: ScoreMode) -> Vec<SearchResult> {
    let mut raw_results: Vec<RawResult> = Vec::new();

    for batch in batches {
        let n = batch.num_rows();
        let bodies = get_string_column(batch, "body");
        let file_paths = get_string_column(batch, "file_path");
        let breadcrumbs = get_string_column(batch, "heading_breadcrumb");
        let line_starts = get_u32_column(batch, "line_start");
        let line_ends = get_u32_column(batch, "line_end");
        let frontmatters = get_string_column(batch, "frontmatter_json");

        // Score columns -- try f32 first, then f64 fallback (Pitfall 4)
        let distances = get_f64_score_column(batch, "_distance");
        let fts_scores = get_f64_score_column(batch, "_score");
        let relevance_scores = get_f64_score_column(batch, "_relevance_score");

        for i in 0..n {
            let fm_json = frontmatters
                .as_ref()
                .and_then(|arr| {
                    if arr.is_null(i) {
                        None
                    } else {
                        serde_json::from_str(arr.value(i)).ok()
                    }
                })
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

            raw_results.push(RawResult {
                body: bodies
                    .as_ref()
                    .map(|a| a.value(i).to_string())
                    .unwrap_or_default(),
                file_path: file_paths
                    .as_ref()
                    .map(|a| a.value(i).to_string())
                    .unwrap_or_default(),
                heading_breadcrumb: breadcrumbs
                    .as_ref()
                    .map(|a| a.value(i).to_string())
                    .unwrap_or_default(),
                line_start: line_starts.as_ref().map(|a| a.value(i)).unwrap_or(0),
                line_end: line_ends.as_ref().map(|a| a.value(i)).unwrap_or(0),
                frontmatter: fm_json,
                distance: distances.as_ref().map(|d| d[i]),
                fts_score_raw: fts_scores.as_ref().map(|s| s[i]),
                relevance_score_raw: relevance_scores.as_ref().map(|s| s[i]),
            });
        }
    }

    // Normalize scores based on mode
    normalize_and_build(raw_results, mode)
}

/// Internal raw result before normalization
struct RawResult {
    body: String,
    file_path: String,
    heading_breadcrumb: String,
    line_start: u32,
    line_end: u32,
    frontmatter: serde_json::Value,
    distance: Option<f64>,
    fts_score_raw: Option<f64>,
    relevance_score_raw: Option<f64>,
}

/// Normalize raw scores and build SearchResults.
fn normalize_and_build(raw: Vec<RawResult>, mode: ScoreMode) -> Vec<SearchResult> {
    if raw.is_empty() {
        return Vec::new();
    }

    // Find max values for normalization
    let max_fts: f64 = raw
        .iter()
        .filter_map(|r| r.fts_score_raw)
        .fold(0.0_f64, f64::max);
    let max_relevance: f64 = raw
        .iter()
        .filter_map(|r| r.relevance_score_raw)
        .fold(0.0_f64, f64::max);

    raw.into_iter()
        .map(|r| {
            let (similarity_score, semantic_score, fts_score) = match mode {
                ScoreMode::Semantic => {
                    // semantic_score = 1.0 - (distance / 2.0)
                    let sem = r.distance.map(|d| normalize_cosine_distance(d)).unwrap_or(0.0);
                    (sem, Some(sem), None)
                }
                ScoreMode::Fts => {
                    // fts_score = score / max_score
                    let fts = if max_fts > 0.0 {
                        r.fts_score_raw.map(|s| s / max_fts).unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    (fts, None, Some(fts))
                }
                ScoreMode::Hybrid => {
                    // All three scores
                    let sem = r.distance.map(|d| normalize_cosine_distance(d));
                    let fts = if max_fts > 0.0 {
                        r.fts_score_raw.map(|s| s / max_fts)
                    } else {
                        r.fts_score_raw.map(|_| 0.0)
                    };
                    let sim = if max_relevance > 0.0 {
                        r.relevance_score_raw
                            .map(|s| s / max_relevance)
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    (sim, sem, fts)
                }
            };

            SearchResult {
                chunk_text: r.body,
                file_path: r.file_path,
                heading_breadcrumb: r.heading_breadcrumb,
                similarity_score,
                semantic_score,
                fts_score,
                line_range: LineRange {
                    start: r.line_start,
                    end: r.line_end,
                },
                frontmatter: r.frontmatter,
                is_context: None,
                context_for_index: None,
            }
        })
        .collect()
}

/// Convert cosine distance (0.0-2.0) to similarity (0.0-1.0).
/// 0.0 distance = 1.0 similarity, 2.0 distance = 0.0 similarity.
fn normalize_cosine_distance(distance: f64) -> f64 {
    (1.0 - (distance / 2.0)).clamp(0.0, 1.0)
}

/// Check if frontmatter JSON contains a specific tag.
fn frontmatter_has_tag(frontmatter: &serde_json::Value, tag: &str) -> bool {
    frontmatter
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|tags| tags.iter().any(|t| t.as_str() == Some(tag)))
        .unwrap_or(false)
}

// -- Arrow column extraction helpers --

fn get_string_column<'b>(batch: &'b RecordBatch, name: &str) -> Option<&'b StringArray> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())
}

fn get_u32_column<'b>(batch: &'b RecordBatch, name: &str) -> Option<&'b UInt32Array> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<UInt32Array>())
}

/// Get a score column as Vec<f64>. Tries Float32 first, then Float64.
/// Returns None if the column doesn't exist.
fn get_f64_score_column(batch: &RecordBatch, name: &str) -> Option<Vec<f64>> {
    let col = batch.column_by_name(name)?;

    // Try Float32 first
    if let Some(f32_arr) = col.as_primitive_opt::<Float32Type>() {
        return Some(f32_arr.values().iter().map(|v| *v as f64).collect());
    }

    // Try Float64
    if let Some(f64_arr) = col.as_primitive_opt::<Float64Type>() {
        return Some(f64_arr.values().iter().map(|v| *v).collect());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_normalization_semantic() {
        // distance = 0.5 -> similarity = 1.0 - (0.5 / 2.0) = 0.75
        let score = normalize_cosine_distance(0.5);
        assert!((score - 0.75).abs() < 1e-10);
    }

    #[test]
    fn test_score_normalization_semantic_extremes() {
        // distance = 0.0 -> similarity = 1.0
        assert!((normalize_cosine_distance(0.0) - 1.0).abs() < 1e-10);
        // distance = 2.0 -> similarity = 0.0
        assert!((normalize_cosine_distance(2.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_score_normalization_fts() {
        // scores [3.0, 6.0, 9.0] -> normalized to [0.333, 0.666, 1.0]
        let raw = vec![
            make_raw_fts(3.0),
            make_raw_fts(6.0),
            make_raw_fts(9.0),
        ];
        let results = normalize_and_build(raw, ScoreMode::Fts);
        assert_eq!(results.len(), 3);

        let expected = [1.0 / 3.0, 2.0 / 3.0, 1.0];
        for (r, e) in results.iter().zip(expected.iter()) {
            assert!(
                (r.similarity_score - e).abs() < 1e-10,
                "expected {}, got {}",
                e,
                r.similarity_score
            );
            assert!(r.fts_score.is_some());
            assert!(r.semantic_score.is_none());
        }
    }

    #[test]
    fn test_score_normalization_fts_single() {
        // single score [5.0] -> normalized to [1.0]
        let raw = vec![make_raw_fts(5.0)];
        let results = normalize_and_build(raw, ScoreMode::Fts);
        assert_eq!(results.len(), 1);
        assert!((results[0].similarity_score - 1.0).abs() < 1e-10);
        assert!(
            (results[0].fts_score.unwrap() - 1.0).abs() < 1e-10
        );
    }

    #[test]
    fn test_tag_filter_logic() {
        let fm = serde_json::json!({"tags": ["rust", "search"]});
        assert!(frontmatter_has_tag(&fm, "rust"));
        assert!(frontmatter_has_tag(&fm, "search"));
        assert!(!frontmatter_has_tag(&fm, "python"));
    }

    #[test]
    fn test_tag_filter_no_tags() {
        let fm = serde_json::json!({});
        assert!(!frontmatter_has_tag(&fm, "rust"));
    }

    #[test]
    fn test_tag_filter_empty_tags() {
        let fm = serde_json::json!({"tags": []});
        assert!(!frontmatter_has_tag(&fm, "rust"));
    }

    // Helper to create a RawResult with just an FTS score
    fn make_raw_fts(score: f64) -> RawResult {
        RawResult {
            body: "body".to_string(),
            file_path: "file.md".to_string(),
            heading_breadcrumb: "# H".to_string(),
            line_start: 1,
            line_end: 5,
            frontmatter: serde_json::json!({}),
            distance: None,
            fts_score_raw: Some(score),
            relevance_score_raw: None,
        }
    }
}
