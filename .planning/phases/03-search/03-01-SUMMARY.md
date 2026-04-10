---
phase: 03-search
plan: 01
subsystem: search
tags: [search, lancedb, vector-search, fts, hybrid, rrf]
dependency_graph:
  requires: [pipeline/store, pipeline/embedder]
  provides: [search/engine, search/types]
  affects: [main.rs (future wiring in 03-02)]
tech_stack:
  added: [lance-index =2.0.0]
  patterns: [generic SearchEngine<E: Embedder>, Arrow RecordBatch extraction, RRF hybrid fusion]
key_files:
  created:
    - src/search/mod.rs
    - src/search/types.rs
    - src/search/engine.rs
  modified:
    - src/lib.rs
    - src/pipeline/store.rs
    - Cargo.toml
decisions:
  - Used generic E: Embedder instead of dyn Embedder (Embedder trait uses impl Future, not dyn-compatible)
  - FTS index rebuilt before each FTS/hybrid search via ensure_fts_index() (idempotent, acceptable for v1 vault sizes)
  - Tag filter uses 3x over-fetch multiplier with post-query Rust filtering (LanceDB SQL has no JSON path queries)
  - Score columns extracted with f32-first, f64-fallback pattern per research Pitfall 4
metrics:
  duration: 19min
  completed: 2026-04-10
  tasks_completed: 2
  tasks_total: 2
  files_created: 3
  files_modified: 3
  tests_added: 11
---

# Phase 03 Plan 01: Core Search Module Summary

SearchEngine with semantic, FTS, and hybrid (RRF k=60) search modes using LanceDB's native query builder, score normalization to 0.0-1.0, path/tag filtering, and context chunk assembly.

## What Was Built

### Task 1: Search Types and Module Structure
- Created `src/search/types.rs` with `SearchResult`, `SearchResponse`, `SearchOptions`, `SearchMode`, `LineRange`
- `SearchResult` uses `#[serde(skip_serializing_if = "Option::is_none")]` for optional scores per D-04
- `SearchResponse` wraps results per D-02 with query, mode, total, results fields
- `SearchMode` enum decoupled from CLI (library-level, no clap dependency)
- Registered `pub mod search` in `src/lib.rs`
- 4 serialization unit tests

### Task 2: SearchEngine Implementation
- Created `src/search/engine.rs` with `SearchEngine<E: Embedder>` (generic, not dyn due to trait design)
- **Semantic search**: Embeds query via Embedder, queries LanceDB with `nearest_to()` + `DistanceType::Cosine`, normalizes distance to similarity via `1.0 - (distance / 2.0)`
- **FTS search**: Ensures FTS index on `body` column, queries via `full_text_search(FullTextSearchQuery)`, normalizes BM25 scores by dividing by max score in result set
- **Hybrid search**: Chains `full_text_search` + `nearest_to` + `RRFReranker(k=60.0)`, produces all three scores (semantic, fts, similarity from `_relevance_score`)
- **Path filter**: SQL LIKE with escaped single quotes (`replace("'", "''")`)
- **Tag filter**: Post-query Rust filtering on deserialized `frontmatter_json`, 3x over-fetch multiplier
- **Context chunks**: Queries same file by `file_path`, sorts by `line_start`, takes N before/after matching chunk, marks with `is_context: true` and `context_for_index`
- **min_score filter**: Applied after search, before context fetch
- Added `table()` getter to `ChunkStore` in `src/pipeline/store.rs`
- Added `lance-index = "=2.0.0"` dependency for `FullTextSearchQuery` type
- 7 engine unit tests (score normalization, tag filtering)

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | 293c549 | feat(03-01): create search types module with SearchResult, SearchResponse, SearchOptions |
| 2 | b6e66ea | feat(03-01): implement SearchEngine with semantic, FTS, and hybrid search modes |

## Decisions Made

1. **Generic vs dyn Embedder**: The `Embedder` trait returns `impl Future` which is not dyn-compatible. Used generic `SearchEngine<E: Embedder>` instead of `&dyn Embedder`.
2. **FTS index per search**: Calling `ensure_fts_index()` before each FTS/hybrid search. This rebuilds the index to include any new data. Acceptable for v1 personal vault sizes (<50K chunks).
3. **Tag filter strategy**: Post-query filtering with 3x over-fetch. LanceDB SQL dialect has no JSON path queries, so `frontmatter_json` must be deserialized in Rust. Simpler than SQL LIKE hack, correct for all cases.
4. **Score column extraction**: Uses `as_primitive_opt::<Float32Type>()` with `Float64Type` fallback per Pitfall 4 from research.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Embedder trait is not dyn-compatible**
- **Found during:** Task 1 (engine placeholder)
- **Issue:** The plan specified `&'a dyn Embedder` but the trait uses `impl Future` return types which prevent dyn dispatch
- **Fix:** Changed to generic `SearchEngine<'a, E: Embedder>` parameter
- **Files modified:** src/search/engine.rs
- **Commit:** 293c549

**2. [Rule 3 - Blocking] lance-index dependency needed for FullTextSearchQuery**
- **Found during:** Task 2
- **Issue:** `FullTextSearchQuery` is defined in `lance_index::scalar`, not re-exported by lancedb
- **Fix:** Added `lance-index = "=2.0.0"` to Cargo.toml (matches lancedb's pinned version)
- **Files modified:** Cargo.toml
- **Commit:** b6e66ea

## Known Stubs

None -- all search modes are fully implemented with real LanceDB queries. No placeholder data or TODO items.

## Self-Check: PASSED

- All 3 created files exist (mod.rs, types.rs, engine.rs)
- Both commits verified (293c549, b6e66ea)
- `cargo check` passes
- 11 unit tests pass (4 types + 7 engine)
