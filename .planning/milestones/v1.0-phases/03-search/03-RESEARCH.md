# Phase 3: Search - Research

**Researched:** 2026-04-10
**Domain:** LanceDB vector + FTS search, RRF hybrid fusion, CLI output formatting
**Confidence:** HIGH

## Summary

Phase 3 implements the `search` command against an existing LanceDB-backed ChunkStore. The LanceDB Rust crate (v0.26.2 in the project, v0.27.2 latest) natively supports both vector ANN search (`nearest_to`) and BM25 full-text search (`full_text_search` with `FullTextSearchQuery`). Crucially, LanceDB also supports hybrid search by chaining both on the same query builder, with built-in `RRFReranker` (k=60 default) that produces a `_relevance_score` column alongside `_distance` (vector) and `_score` (FTS).

The existing `ChunkStore` in `src/pipeline/store.rs` holds the LanceDB `Connection` and `Table`. Search methods should be added directly to `ChunkStore` (or a thin `SearchEngine` wrapper) since it already owns the table handle. The FTS index must be created once on the `body` column before FTS queries work -- this is a one-time `create_index` call. The query embedding for semantic/hybrid modes reuses the existing `VoyageEmbedder`. All CLI flags (`--mode`, `--limit`, `--min-score`, `--path-filter`, `--tag-filter`, `--context`, `--format`) are already defined in `src/cli.rs`.

**Primary recommendation:** Use LanceDB's built-in hybrid search pipeline (`nearest_to` + `full_text_search` + `RRFReranker`) rather than running two separate queries and fusing manually. This gives us RRF for free with `_relevance_score`, `_distance`, and `_score` columns in the output RecordBatch.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use LanceDB's built-in FTS for BM25 full-text search. Single table, no extra files, no tantivy dependency. Fallback to tantivy only if show-stopper bug.
- **D-02:** JSON output is a wrapped object: `{"query": ..., "mode": ..., "total": N, "results": [...]}`
- **D-03:** Each result contains: `chunk_text`, `file_path` (vault-relative), `heading_breadcrumb`, `similarity_score` (fused RRF), `semantic_score`, `fts_score`, `line_range` (`{"start": N, "end": N}`), `frontmatter` (`{"tags": [...], "aliases": [...], ...}`)
- **D-04:** In hybrid mode report all three scores (similarity_score, semantic_score, fts_score). In single mode, omit the score for the unused mode (or null).
- **D-05:** `--format pretty` renders snippet blocks with `=` separator, ~200 char truncation, vault-relative path.
- **D-06:** RRF k constant is Claude's discretion (standard: k=60). Normalize scores to 0.0-1.0.
- **D-07:** Context chunks use `is_context: true` and `context_for_index` in flat results array.
- **D-08:** Query embedding uses `VoyageEmbedder` directly (no HyDE).

### Claude's Discretion
- LanceDB FTS index creation API (create_index call, tokenizer options -- use defaults)
- RRF k constant (standard: 60)
- Exact BM25 score normalization approach
- Context chunk representation details
- Error messages for "index not found"
- indicatif/progress behavior during search (probably none)

### Deferred Ideas (OUT OF SCOPE)
- HyDE (hypothetical document expansion) -- deferred to v2
- LLM reranking -- deferred
- Query expansion / synonyms -- v2
- Streaming results -- v1 returns all at once
- Tantivy sidecar index -- LanceDB FTS is primary; tantivy only if show-stopper
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-03 | `local-index search "<query>"` with structured JSON output | CLI already defined in `src/cli.rs`; implement handler in `main.rs` |
| SRCH-01 | Semantic (vector ANN) queries via LanceDB | `table.query().nearest_to(&vector)` with `_distance` column |
| SRCH-02 | Full-text queries over chunk text | `table.query().full_text_search(FullTextSearchQuery::new(q))` with `_score` column |
| SRCH-03 | Hybrid mode with RRF fusion (default) | Chain `nearest_to` + `full_text_search` + `rerank(RRFReranker)` on same query; `_relevance_score` output |
| SRCH-04 | Structured JSON output with required fields | Serde structs for SearchResponse/SearchResult; map Arrow RecordBatch columns to struct fields |
| SRCH-05 | `--limit N` and `--min-score F` flags | `.limit(n)` on query builder; post-filter `_relevance_score >= min_score` |
| SRCH-06 | `--mode [semantic\|fts\|hybrid]` flag | Already defined as `SearchMode` enum; dispatch to different query paths |
| SRCH-07 | `--path-filter <prefix>` for path prefix filtering | `.only_if("file_path LIKE 'prefix%'")` on query builder |
| SRCH-08 | `--tag-filter <tag>` for frontmatter tag filtering | Parse `frontmatter_json` post-query or use SQL predicate on JSON column |
| SRCH-09 | `--format [json\|pretty]` output selection | Already defined as `OutputFormat` enum; two rendering paths |
| SRCH-10 | `--context N` for surrounding chunks | Separate query for adjacent chunks by file_path + line_range proximity |
</phase_requirements>

## Standard Stack

### Core (already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| lancedb | 0.26.2 (locked) | Vector + FTS + hybrid search | Project's embedded DB; has native FTS and RRF reranker |
| arrow-array | 57 (locked) | Record batch column extraction | Already used in store.rs for Arrow data |
| arrow-schema | 57 (locked) | Schema types | Already a dependency |
| serde | 1.0 | JSON serialization | Search result structs |
| serde_json | 1.0 | JSON output | Serialize SearchResponse to stdout |
| reqwest | 0.12 | Voyage API (query embedding) | Used by VoyageEmbedder |
| futures | 0.3 | Async stream collection | `TryStreamExt` for RecordBatch streams |

### No New Dependencies Needed
All required functionality is available through existing dependencies. LanceDB 0.26.2 includes FTS, vector search, `RRFReranker`, and `FullTextSearchQuery`. No new crates are needed for Phase 3.

### Version Note
The project locks lancedb at 0.26.2. Latest is 0.27.2. The FTS, vector search, and RRF APIs are stable across both versions. No upgrade needed for Phase 3 functionality.

## Architecture Patterns

### Recommended Project Structure
```
src/
  pipeline/
    store.rs          # Add search methods to ChunkStore (FTS index creation, search queries)
  search/
    mod.rs            # SearchEngine orchestrator (query dispatch, score normalization, context assembly)
    types.rs          # SearchResponse, SearchResult, SearchOptions structs
    formatter.rs      # JSON and pretty output formatting
  cli.rs              # Existing (no changes needed)
  main.rs             # Wire Command::Search to SearchEngine
  lib.rs              # Add `pub mod search;`
  error.rs            # Add SearchError variant if needed
```

### Pattern 1: SearchEngine Orchestrator
**What:** A `SearchEngine` struct that holds a `ChunkStore` reference and a `VoyageEmbedder`, dispatching to the right query path based on `SearchMode`.
**When to use:** All search operations go through this single entry point.
**Example:**
```rust
// Source: project architecture pattern
pub struct SearchEngine<'a> {
    store: &'a ChunkStore,
    embedder: &'a dyn Embedder,
}

impl<'a> SearchEngine<'a> {
    pub async fn search(&self, opts: &SearchOptions) -> Result<SearchResponse, LocalIndexError> {
        match opts.mode {
            SearchMode::Semantic => self.semantic_search(opts).await,
            SearchMode::Fts => self.fts_search(opts).await,
            SearchMode::Hybrid => self.hybrid_search(opts).await,
        }
    }
}
```

### Pattern 2: Arrow RecordBatch to Rust Structs
**What:** Extract typed values from Arrow RecordBatch columns returned by LanceDB queries.
**When to use:** Every search result needs mapping from Arrow to `SearchResult`.
**Example:**
```rust
// Source: existing pattern in store.rs get_hashes_for_file
fn extract_search_results(batches: &[RecordBatch]) -> Vec<SearchResult> {
    let mut results = Vec::new();
    for batch in batches {
        let body_col = batch.column_by_name("body")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let file_path_col = batch.column_by_name("file_path")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let score_col = batch.column_by_name("_relevance_score")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>());
        // ... map rows to SearchResult structs
    }
    results
}
```

### Pattern 3: FTS Index Lazy Creation
**What:** Create the FTS index on the `body` column the first time a search (FTS or hybrid) is performed. Check if index exists first.
**When to use:** FTS and hybrid search modes.
**Example:**
```rust
// Source: LanceDB docs (https://docs.lancedb.com/search/full-text-search)
pub async fn ensure_fts_index(&self) -> Result<(), LocalIndexError> {
    // LanceDB create_index is idempotent -- calling it when index exists
    // will recreate it. Check index list first or handle error.
    self.table
        .create_index(&["body"], Index::FTS(FtsIndexBuilder::default()))
        .execute()
        .await
        .map_err(|e| LocalIndexError::Database(e.to_string()))?;
    Ok(())
}
```

### Anti-Patterns to Avoid
- **Running two separate queries for hybrid search:** LanceDB supports chaining `nearest_to` + `full_text_search` on the same query builder with `rerank(RRFReranker)`. Do NOT run vector and FTS queries separately and merge manually -- the built-in pipeline handles rank fusion, deduplication, and score normalization.
- **Storing FTS in a separate tantivy index:** D-01 locks us to LanceDB FTS. No tantivy unless show-stopper.
- **Parsing frontmatter_json in SQL:** LanceDB's SQL dialect does not support JSON path queries. Tag filtering must be done post-query by deserializing `frontmatter_json` and checking tag membership in Rust.
- **Forgetting to create FTS index:** FTS queries will fail if the index has not been created. The search command must ensure the index exists before any FTS/hybrid query.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RRF fusion | Custom rank fusion loop | `lancedb::rerankers::rrf::RRFReranker` | Built-in, handles dedup, score columns, edge cases |
| BM25 scoring | Custom BM25 implementation | LanceDB FTS (`_score` column) | tantivy-based internally, returns BM25 scores |
| Vector distance | Manual cosine similarity | LanceDB `nearest_to` (`_distance` column) | Uses optimized SIMD, handles flat/IVF search |
| Score normalization to 0-1 | Complex min-max normalization | RRFReranker `_relevance_score` + simple 1/(1+d) for distance | RRF scores are already relative; distance needs simple transform |

**Key insight:** LanceDB's hybrid search pipeline does the heavy lifting. The main implementation work is (a) wiring the query builder correctly, (b) extracting Arrow columns into Rust structs, and (c) formatting output.

## LanceDB Search API Details

### Vector Search (Semantic Mode)
```rust
// Source: https://docs.rs/lancedb/latest/lancedb/query/struct.VectorQuery.html
let query_vector: Vec<f32> = embedder.embed(&[query_text]).await?.embeddings.remove(0);

let results: Vec<RecordBatch> = store.table
    .query()
    .nearest_to(&query_vector)?
    .distance_type(DistanceType::Cosine)  // Voyage embeddings use cosine
    .limit(limit)
    .only_if("file_path LIKE 'notes/%'")  // optional path filter
    .execute()
    .await?
    .try_collect()
    .await?;
// Results include `_distance` column (lower = more similar)
// For cosine distance: 0.0 = identical, 2.0 = opposite
```

### FTS Search (Full-Text Mode)
```rust
// Source: https://docs.lancedb.com/search/full-text-search
// FTS index must exist first:
store.table
    .create_index(&["body"], Index::FTS(FtsIndexBuilder::default()))
    .execute()
    .await?;

let results: Vec<RecordBatch> = store.table
    .query()
    .full_text_search(FullTextSearchQuery::new(query_text.to_owned()))
    .limit(limit)
    .only_if("file_path LIKE 'notes/%'")
    .execute()
    .await?
    .try_collect()
    .await?;
// Results include `_score` column (BM25 score, higher = more relevant)
```

### Hybrid Search (Default Mode)
```rust
// Source: https://docs.rs/lancedb/latest/lancedb/query/struct.Query.html + RRF docs
use lancedb::rerankers::rrf::RRFReranker;
use std::sync::Arc;

let query_vector: Vec<f32> = embedder.embed(&[query_text]).await?.embeddings.remove(0);

let results: Vec<RecordBatch> = store.table
    .query()
    .full_text_search(FullTextSearchQuery::new(query_text.to_owned()))
    .nearest_to(&query_vector)?
    .distance_type(DistanceType::Cosine)
    .rerank(Arc::new(RRFReranker::new(60.0)))
    .limit(limit)
    .only_if("file_path LIKE 'notes/%'")
    .execute()
    .await?
    .try_collect()
    .await?;
// Results include: _distance, _score, _relevance_score columns
// _relevance_score is the fused RRF score (higher = more relevant)
```

### Score Columns Summary

| Column | Present In | Meaning | Range |
|--------|-----------|---------|-------|
| `_distance` | Vector, Hybrid | Cosine distance (lower = better) | 0.0 to 2.0 |
| `_score` | FTS, Hybrid | BM25 relevance (higher = better) | 0.0 to unbounded |
| `_relevance_score` | Hybrid (after RRF) | Fused RRF score (higher = better) | 0.0 to ~0.03 (RRF range) |

### Score Normalization Strategy

**For D-04 (all scores normalized to 0.0-1.0):**

1. **semantic_score** (from `_distance`): `1.0 - (_distance / 2.0)` for cosine distance. This maps 0.0 distance to 1.0 similarity and 2.0 distance to 0.0 similarity.

2. **fts_score** (from `_score`): BM25 scores are unbounded. Normalize within the result set: `score / max_score_in_results`. If only one result, score = 1.0. This gives relative ranking within the query.

3. **similarity_score** (from `_relevance_score` in hybrid, or the single-mode score): In hybrid mode, normalize `_relevance_score` the same way (divide by max in result set). In single modes, copy the normalized semantic or FTS score.

**Recommendation for RRF k:** Use k=60 (standard default, matches LanceDB's own default).

### Filtering

**Path prefix filter (`--path-filter`):**
```rust
// LanceDB supports SQL-like WHERE clauses
.only_if(format!("file_path LIKE '{}%'", path_prefix))
```

**Tag filter (`--tag-filter`):** LanceDB does not support JSON path queries in its SQL dialect. The `frontmatter_json` column stores serialized JSON. Two approaches:

1. **Post-query filtering (recommended):** Fetch extra results (e.g., 3x limit), deserialize `frontmatter_json` in Rust, filter by tag membership, then truncate to limit. Simple, correct, no SQL dialect dependency.

2. **SQL LIKE hack:** `.only_if(format!("frontmatter_json LIKE '%\"{}%'", tag))` -- fragile, could match partial strings.

**Recommendation:** Post-query filtering for tags. Over-fetch with a multiplier, then filter in Rust.

### Context Chunks (`--context N`)

Context chunks are adjacent chunks from the same file. After getting search results, for each result with `context > 0`:

1. Query the same file's chunks: `.only_if(format!("file_path = '{}'", result.file_path))`
2. Sort by `line_start`
3. Find the matching chunk by line range
4. Return N chunks before and N chunks after in the ordered list

Mark context chunks with `is_context: true` and `context_for_index: <index of match>` per D-07.

**Important:** Context query does NOT go through the search pipeline -- it's a plain table scan filtered by file_path. This is a separate `table.query()` call without `nearest_to` or `full_text_search`.

## Common Pitfalls

### Pitfall 1: FTS Index Not Created
**What goes wrong:** FTS and hybrid queries silently return empty results or error if no FTS index exists on the `body` column.
**Why it happens:** The FTS index is not created during `store_chunks` (Phase 2). It must be created before the first FTS query.
**How to avoid:** Call `ensure_fts_index()` at the start of any FTS or hybrid search. Use `create_index` which is idempotent in LanceDB -- calling it when the index exists just rebuilds it (fast for small datasets). Alternatively, check if index exists and skip if so.
**Warning signs:** Empty results for queries that should match; "index not found" errors.

### Pitfall 2: Cosine Distance vs. Similarity Confusion
**What goes wrong:** Reporting `_distance` as similarity score (lower distance = higher similarity, but users expect higher = better).
**Why it happens:** LanceDB returns cosine **distance** (0-2), not cosine **similarity** (-1 to 1 or 0 to 1).
**How to avoid:** Always convert: `similarity = 1.0 - (distance / 2.0)`. Document this transform clearly.
**Warning signs:** Top results have the lowest scores; `--min-score 0.5` filters out everything.

### Pitfall 3: Tag Filter SQL Injection / Escaping
**What goes wrong:** Path or tag filter values containing single quotes break SQL `only_if` predicates.
**Why it happens:** String interpolation into SQL without escaping.
**How to avoid:** Escape single quotes by doubling them: `value.replace("'", "''")`. Or use parameterized queries if LanceDB supports them (it may not -- verify).
**Warning signs:** Panics or database errors when filter values contain quotes or special characters.

### Pitfall 4: Arrow Column Type Mismatch
**What goes wrong:** `downcast_ref::<Float32Array>()` returns `None` when score column is actually `Float64Array`.
**Why it happens:** LanceDB may return `_distance`, `_score`, or `_relevance_score` as f32 or f64 depending on the query type and version.
**How to avoid:** Try both f32 and f64 downcasts, or use `as_primitive_opt::<Float32Type>()` with fallback to `Float64Type`.
**Warning signs:** "column not found" or unwrap panics during result extraction.

### Pitfall 5: Empty Database Search
**What goes wrong:** Search crashes or errors when the LanceDB table exists but has zero rows.
**Why it happens:** FTS index creation on empty table, or vector search with no data.
**How to avoid:** Check row count or handle gracefully: return `SearchResponse { total: 0, results: vec![] }`. The empty case should be tested explicitly.
**Warning signs:** Panics on `unwrap()` or division by zero in normalization.

### Pitfall 6: Over-fetching for Tag Filter
**What goes wrong:** Tag-filtered search returns fewer results than `--limit` because post-filtering removes too many.
**Why it happens:** Fixed over-fetch multiplier (e.g., 3x) may not be enough for rare tags.
**How to avoid:** Use a loop: fetch batch, filter, if not enough results and more available, fetch next batch. Or accept that tag-filtered results may return fewer than `--limit` (simpler, probably fine for v1).
**Warning signs:** `--limit 10 --tag-filter rare-tag` returns 2 results when 5 exist.

## Code Examples

### SearchResult Struct
```rust
// Source: D-03 from CONTEXT.md
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub chunk_text: String,
    pub file_path: String,
    pub heading_breadcrumb: String,
    pub similarity_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fts_score: Option<f64>,
    pub line_range: LineRange,
    pub frontmatter: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_context: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_for_index: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub mode: String,
    pub total: usize,
    pub results: Vec<SearchResult>,
}
```

### Pretty Output Format
```rust
// Source: D-05 from CONTEXT.md
fn format_pretty(response: &SearchResponse) -> String {
    let mut out = String::new();
    for (i, result) in response.results.iter().enumerate() {
        let truncated = if result.chunk_text.len() > 200 {
            format!("{}[truncated]", &result.chunk_text[..200])
        } else {
            result.chunk_text.clone()
        };
        out.push_str(&format!(
            "[{}] {} -- {}  (score: {:.2})\n",
            i + 1, result.file_path, result.heading_breadcrumb, result.similarity_score
        ));
        out.push_str(&"=".repeat(40));
        out.push('\n');
        out.push_str(&format!("{}\n{}\n\n", result.heading_breadcrumb, truncated));
    }
    out
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate tantivy + LanceDB | LanceDB built-in FTS | LanceDB 0.20+ | No external FTS dependency needed |
| Manual RRF in application code | LanceDB `RRFReranker` | LanceDB 0.24+ | Built-in hybrid search pipeline |
| Python-only FTS | Rust FTS via `FullTextSearchQuery` | LanceDB 0.22+ | Full FTS support in Rust crate |

**Deprecated/outdated:**
- `vectordb` crate: Old LanceDB crate name, replaced by `lancedb`
- Manual tantivy FTS alongside LanceDB: Unnecessary since LanceDB FTS works natively in Rust

## Open Questions

1. **FTS index rebuild on new data**
   - What we know: `create_index` rebuilds the FTS index. New data added after index creation is NOT automatically indexed for FTS.
   - What's unclear: Whether `create_index` is fast enough to call before every search, or if we should track dirty state and rebuild only when new data has been indexed.
   - Recommendation: For v1, call `create_index` before each FTS/hybrid search. If this is too slow for large vaults (>100K chunks), optimize later with dirty tracking. For personal vaults (<50K chunks) this should complete in under a second.

2. **Arrow Float type for score columns**
   - What we know: LanceDB returns `_distance`, `_score`, `_relevance_score` but the exact Arrow type (f32 vs f64) is not documented.
   - What's unclear: Whether the type varies by query mode or version.
   - Recommendation: Implement extraction with fallback: try Float32Array, then Float64Array, then error. Test with actual queries in integration tests.

3. **`only_if` SQL dialect for path prefix**
   - What we know: LanceDB supports `LIKE` for string matching.
   - What's unclear: Whether `LIKE` with `%` wildcard works correctly for path prefix filtering.
   - Recommendation: Test `only_if("file_path LIKE 'notes/%'")` in integration tests. If LIKE is not supported, fall back to `starts_with` post-filtering.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + tokio::test |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SRCH-01 | Semantic vector search returns ranked results | integration | `cargo test --test search_integration::test_semantic_search -x` | Wave 0 |
| SRCH-02 | FTS BM25 search returns ranked results | integration | `cargo test --test search_integration::test_fts_search -x` | Wave 0 |
| SRCH-03 | Hybrid RRF fusion returns fused results | integration | `cargo test --test search_integration::test_hybrid_search -x` | Wave 0 |
| SRCH-04 | JSON output contains all required fields | unit | `cargo test search::types::tests::test_search_result_serialization -x` | Wave 0 |
| SRCH-05 | --limit and --min-score filtering | unit | `cargo test search::tests::test_limit_and_min_score -x` | Wave 0 |
| SRCH-06 | --mode dispatches correctly | unit | `cargo test search::tests::test_mode_dispatch -x` | Wave 0 |
| SRCH-07 | --path-filter restricts by prefix | integration | `cargo test --test search_integration::test_path_filter -x` | Wave 0 |
| SRCH-08 | --tag-filter restricts by tag | integration | `cargo test --test search_integration::test_tag_filter -x` | Wave 0 |
| SRCH-09 | --format json and --format pretty output | unit | `cargo test search::formatter::tests -x` | Wave 0 |
| SRCH-10 | --context N includes surrounding chunks | integration | `cargo test --test search_integration::test_context_chunks -x` | Wave 0 |
| CLI-03 | CLI search command parses and executes | integration | `cargo test --test cli_integration::test_search_help -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before verify

### Wave 0 Gaps
- [ ] `tests/search_integration.rs` -- integration tests for all search modes against real LanceDB (tempdir)
- [ ] `src/search/types.rs` -- unit tests for serialization, score normalization
- [ ] `src/search/formatter.rs` -- unit tests for pretty and JSON formatting

## Project Constraints (from CLAUDE.md)

- **Rust only** -- no Node/Python helpers
- **LanceDB embedded** -- no external database process
- **clap derive macros** for CLI
- **tracing crate** for logging (no `log` crate)
- **anyhow** in binary, **thiserror** in library code
- **serde/serde_json** for JSON
- Logs to stderr, output to stdout
- Per-result graceful error handling: warn and skip, never abort search

## Sources

### Primary (HIGH confidence)
- [docs.rs/lancedb](https://docs.rs/lancedb/latest/lancedb/) -- Rust crate API reference (VectorQuery, QueryBase, FullTextSearchQuery, RRFReranker)
- [LanceDB FTS docs](https://docs.lancedb.com/search/full-text-search) -- FTS creation, `_score` column, BM25, filtering
- [LanceDB VectorQuery docs](https://docs.rs/lancedb/latest/lancedb/query/struct.VectorQuery.html) -- `nearest_to`, `_distance`, `distance_type`
- [LanceDB RRFReranker docs](https://docs.rs/lancedb/latest/lancedb/rerankers/rrf/struct.RRFReranker.html) -- k parameter, `rerank_hybrid`, `_relevance_score`
- [LanceDB FtsIndexBuilder docs](https://docs.rs/lancedb/latest/lancedb/index/scalar/struct.FtsIndexBuilder.html) -- Tokenizer options, configuration

### Secondary (MEDIUM confidence)
- [DeepWiki LanceDB Hybrid Search](https://deepwiki.com/lancedb/lancedb/6.4-hybrid-search) -- Hybrid query architecture, chaining pattern
- [LanceDB Hybrid Search docs](https://docs.lancedb.com/search/hybrid-search) -- RRF as default, normalization methods
- [DeepWiki Query System](https://deepwiki.com/lancedb/lancedb/6-query-and-search-system) -- Query type selection, HybridQuery struct details

### Tertiary (LOW confidence)
- Score column types (f32 vs f64) -- inferred from docs, needs runtime verification
- `create_index` idempotency behavior -- stated in docs but needs testing with 0.26.2

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all dependencies already in Cargo.toml, LanceDB APIs confirmed in docs
- Architecture: HIGH -- query builder pattern well documented, existing ChunkStore provides clear integration point
- Pitfalls: MEDIUM -- score column types and FTS index rebuild behavior need runtime verification
- LanceDB hybrid API: MEDIUM -- Rust hybrid search docs are thinner than Python; chaining pattern confirmed via source code analysis but not tested

**Research date:** 2026-04-10
**Valid until:** 2026-05-10 (LanceDB APIs are stable post-0.24)
