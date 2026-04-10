---
phase: 03-search
verified: 2026-04-10T00:00:00Z
status: passed
score: 10/10 must-haves verified
---

# Phase 03: Search Verification Report

**Phase Goal:** Implement semantic, full-text, and hybrid search over the indexed vault; expose via `local-index search` CLI with JSON and pretty output.
**Verified:** 2026-04-10
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SearchEngine dispatches to semantic, fts, or hybrid based on SearchMode | VERIFIED | `engine.rs:40-44` — match on `opts.mode` dispatches to each method |
| 2 | Semantic search returns results with semantic_score derived from cosine distance | VERIFIED | `engine.rs:469-473` — `1.0 - (distance / 2.0)`, copied to both `similarity_score` and `semantic_score` |
| 3 | FTS search returns results with fts_score normalized to 0.0-1.0 | VERIFIED | `engine.rs:474-481` — `fts_score_raw / max_fts`, `similarity_score = fts_score` |
| 4 | Hybrid search returns results with all three scores | VERIFIED | `engine.rs:483-499` — semantic, fts, and relevance_score all populated |
| 5 | Path prefix filter restricts results to matching file paths | VERIFIED | `engine.rs:108-111` — SQL LIKE `'{}%'` with quote escaping |
| 6 | Tag filter restricts results to chunks with matching frontmatter tag | VERIFIED | `engine.rs:126-128` — post-query Rust filter via `frontmatter_has_tag`, 3x over-fetch |
| 7 | Context chunks are fetched for adjacent chunks in same file | VERIFIED | `engine.rs:252-368` — queries file by path, sorts by line_start, takes N before/after |
| 8 | min_score threshold filters out low-scoring results | VERIFIED | `engine.rs:47-49` — `results.retain(|r| r.similarity_score >= min_score)` |
| 9 | Operator can run `local-index search "query"` and receive JSON output | VERIFIED | `main.rs:346-417` — full wiring; binary help output confirms all flags |
| 10 | FTS index created during `index` command | VERIFIED | `main.rs:327-336` — `ensure_fts_index()` called after `chunks_embedded > 0` |

**Score:** 10/10 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/search/types.rs` | SearchResult, SearchResponse, SearchOptions, LineRange, SearchMode structs | VERIFIED | 214 lines; all structs present with correct fields and serde skip_serializing_if |
| `src/search/engine.rs` | SearchEngine with semantic_search, fts_search, hybrid_search, ensure_fts_index | VERIFIED | 658 lines; all methods implemented with real LanceDB queries |
| `src/search/formatter.rs` | format_json, format_pretty | VERIFIED | 250 lines; both functions implemented with 5 unit tests |
| `src/search/mod.rs` | Module re-exports | VERIFIED | All four re-exports present (SearchEngine, format_json, format_pretty, SearchMode, SearchOptions, SearchResponse, SearchResult) |
| `src/lib.rs` | pub mod search | VERIFIED | Line 4: `pub mod search;` |
| `src/main.rs` | Search command wiring | VERIFIED | Full wiring at lines 346-417 |
| `tests/search_integration.rs` | Integration test suite | VERIFIED | 14784 bytes; 9 tests all passing |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/search/engine.rs` | `src/pipeline/store.rs` | `store.table()` for LanceDB queries | VERIFIED | `engine.rs:99,153,214` — `self.store.table().query()` in all three search methods |
| `src/search/engine.rs` | `src/pipeline/embedder.rs` | `embedder.embed()` for query embedding | VERIFIED | `engine.rs:82,196` — called in semantic_search and hybrid_search |
| `src/search/engine.rs` | lancedb::query | `nearest_to`, `full_text_search`, `rerank` | VERIFIED | `engine.rs:102,155,216-220` — all three LanceDB query APIs used |
| `src/main.rs` | `src/search/engine.rs` | `SearchEngine::new + search()` | VERIFIED | `main.rs:388,407` |
| `src/main.rs` | `src/search/formatter.rs` | `format_json` or `format_pretty` | VERIFIED | `main.rs:411,413` |
| `src/main.rs` (index cmd) | `src/search/engine.rs` | `ensure_fts_index` after storing chunks | VERIFIED | `main.rs:330-331` |
| `src/pipeline/store.rs` | (table getter) | `pub fn table(&self)` | VERIFIED | `store.rs:295` — getter exposed |

### Data-Flow Trace (Level 4)

The search module does not render data in a web component — it receives query inputs via CLI args and queries LanceDB directly. Data flows:

1. CLI args → `SearchOptions` struct → `SearchEngine::search()` → LanceDB query → Arrow `RecordBatch` → `extract_results_from_batches()` → `Vec<SearchResult>` → formatter → stdout

Each step has been verified substantive (no static returns, no hardcoded empty data). The `extract_results_from_batches` function reads from actual Arrow column arrays returned by LanceDB.

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `engine.rs::semantic_search` | `batches` from LanceDB | `self.store.table().query().nearest_to(...).execute()` | Yes — LanceDB vector ANN query | FLOWING |
| `engine.rs::fts_search` | `batches` from LanceDB | `self.store.table().query().full_text_search(...).execute()` | Yes — LanceDB BM25 query | FLOWING |
| `engine.rs::hybrid_search` | `batches` from LanceDB | `full_text_search + nearest_to + RRFReranker` | Yes — LanceDB hybrid RRF query | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Binary compiles | `cargo check` | Finished dev profile, no errors | PASS |
| Unit tests pass | `cargo test search` | 16 passed, 0 failed | PASS |
| Integration tests pass | `cargo test --test search_integration` | 9 passed, 0 failed | PASS |
| Help output shows all flags | `./target/debug/local-index search --help` | All 7 flags visible: --limit, --min-score, --mode, --path-filter, --tag-filter, --context, --format | PASS |
| Default mode is hybrid | Help output | `[default: hybrid]` shown for --mode flag | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CLI-03 | 03-02 | `local-index search "<query>"` returns JSON on stdout | SATISFIED | `main.rs:346-417` wires full search command; confirmed via help output and integration tests |
| SRCH-01 | 03-01 | Semantic vector ANN search | SATISFIED | `engine.rs:77-134` — `nearest_to()` + cosine distance; `test_semantic_search` passes |
| SRCH-02 | 03-01 | Full-text search | SATISFIED | `engine.rs:137-185` — `full_text_search(FullTextSearchQuery)` + BM25 normalization; `test_fts_search` passes |
| SRCH-03 | 03-01 | Hybrid RRF fusion; hybrid is default | SATISFIED | `engine.rs:188-250` — `RRFReranker::new(60.0)`; `cli.rs:75` `default_value = "hybrid"`; `test_hybrid_search` passes |
| SRCH-04 | 03-01, 03-02 | JSON fields: chunk_text, file_path, heading_breadcrumb, similarity_score, line_range, frontmatter | SATISFIED | `types.rs:12-39` all fields present; `test_json_output_shape` verifies serialization |
| SRCH-05 | 03-02 | `--limit N` (default 10) and `--min-score F` (default none) | SATISFIED | `cli.rs:67-72`; `engine.rs:47-49` min_score applied; `test_limit_flag` passes |
| SRCH-06 | 03-02 | `--mode [semantic\|fts\|hybrid]` flag | SATISFIED | `cli.rs:74-76` with clap ValueEnum; mode dispatch in `engine.rs:40-44` |
| SRCH-07 | 03-02 | `--path-filter <prefix>` | SATISFIED | `cli.rs:79-80`; SQL LIKE in all three search methods; `test_path_filter` passes |
| SRCH-08 | 03-02 | `--tag-filter <tag>` | SATISFIED | `cli.rs:83-84`; post-query Rust filtering with 3x over-fetch; `test_tag_filter` passes |
| SRCH-09 | 03-02 | `--format [json\|pretty]`; json default | SATISFIED | `cli.rs:90-92`; `format_json`/`format_pretty` dispatch in `main.rs:410-413`. Note: REQUIREMENTS.md says "table" but D-05 decision documents snippet blocks as the chosen format to avoid column alignment issues on narrow terminals. The behavior satisfies the requirement's intent. |
| SRCH-10 | 03-01, 03-02 | `--context N` includes surrounding chunks | SATISFIED | `engine.rs:252-368` `fetch_context_chunks`; `cli.rs:87-88`; `test_context_chunks` passes |

All 11 requirement IDs covered. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

Scan confirmed: no TODO/FIXME/PLACEHOLDER/stub comments in any search module file. No empty return values that reach rendering. No hardcoded empty data in search paths. The `SearchMode` import decision (using separate enum in library vs CLI) is documented and intentional.

### Human Verification Required

#### 1. Pretty Output on Narrow Terminal

**Test:** Run `local-index search "query" --format pretty` against a real indexed vault on an 80-column terminal.
**Expected:** Snippet blocks render without misalignment; Unicode ═ separator displays correctly; truncation at 200 chars is clean.
**Why human:** Terminal rendering, visual alignment, and Unicode display cannot be verified programmatically.

#### 2. FTS Index Persists Across Invocations

**Test:** Run `local-index index <vault>`, then in a separate shell run `local-index search "word" --mode fts`. Verify the FTS search completes without rebuilding the index from scratch.
**Expected:** FTS results returned promptly without visible index-creation delay.
**Why human:** Requires a running daemon/persistent state — cannot test with static file checks.

#### 3. Empty Index Error Message Quality

**Test:** Run `local-index search "query"` in a directory with no `.local-index/` folder.
**Expected:** Clear error message: "No index found at '.local-index'. Run `local-index index <path>` first, or specify --data-dir."
**Why human:** Error UX quality judgment; though the message text is verified in code, the actual terminal output and exit code need runtime confirmation.

### Gaps Summary

No gaps found. All phase goals achieved.

---

_Verified: 2026-04-10_
_Verifier: Claude (gsd-verifier)_
