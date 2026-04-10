---
phase: 03-search
plan: 02
subsystem: search-cli
tags: [search, cli, formatter, json, pretty, fts-index]
dependency_graph:
  requires: [search/engine, search/types, pipeline/store, pipeline/embedder, credentials]
  provides: [search/formatter, cli-search-wiring]
  affects: [main.rs]
tech_stack:
  added: []
  patterns: [format dispatch via OutputFormat enum, FTS index creation during indexing]
key_files:
  created:
    - src/search/formatter.rs
    - tests/search_integration.rs
  modified:
    - src/search/mod.rs
    - src/main.rs
decisions:
  - Always require VOYAGE_API_KEY even for FTS-only mode (simplifies v1, avoids conditional embedder construction)
  - FTS index created at end of index command (when chunks_embedded > 0) so search is fast out of the box
  - Pretty format truncates at 200 chars with [truncated] marker and uses Unicode box drawing separator
  - Context chunks rendered with [ctx] prefix in pretty output
metrics:
  duration: 8min
  completed: 2026-04-10
  tasks_completed: 3
  files_changed: 4
---

# Phase 3 Plan 2: Search CLI Wiring and Output Formatting Summary

**One-liner:** Wire SearchEngine into CLI search command with JSON/pretty formatters and FTS index creation during indexing.

## What Was Built

### Task 1: Output Formatters (src/search/formatter.rs)
- `format_json()` -- serializes SearchResponse to pretty-printed JSON with query/mode/total/results wrapper
- `format_pretty()` -- renders snippet blocks with numbered indices, Unicode separator, heading breadcrumbs, score display, 200-char truncation, [ctx] prefix for context chunks, empty result handling
- 5 unit tests covering JSON shape, snippet blocks, truncation, empty results, context chunk rendering

### Task 2: Search Command Wiring (src/main.rs)
- Replaced "search command not yet implemented" placeholder with full SearchEngine integration
- Resolves data-dir, opens ChunkStore, creates VoyageEmbedder, constructs SearchOptions from CLI args
- Dispatches to format_json or format_pretty based on --format flag
- Added "No index found" error when data-dir does not exist
- Added FTS index creation at end of index command (after chunks_embedded > 0) with graceful fallback on failure

### Task 3: Integration Tests (tests/search_integration.rs)
- MockEmbedder returns deterministic normalized 1024-dim vectors without API calls
- 9 integration tests against real LanceDB in tempdirs:
  - test_semantic_search: verifies semantic_score populated, fts_score absent
  - test_fts_search: verifies FTS results match query text, fts_score populated
  - test_hybrid_search: verifies hybrid mode returns results with similarity_score
  - test_json_output_shape: verifies JSON has query/mode/total/results keys
  - test_limit_flag: verifies limit=2 caps results
  - test_path_filter: verifies path prefix filtering
  - test_tag_filter: verifies frontmatter tag filtering
  - test_context_chunks: verifies context=1 returns adjacent chunks with is_context=true
  - test_empty_index_returns_empty_response: verifies empty index returns 0 results, no error

## Deviations from Plan

None -- plan executed exactly as written.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | 29cb6c2 | feat(03-02): implement JSON and pretty output formatters |
| 2 | 7cacbc8 | feat(03-02): wire search command and FTS index creation in main.rs |
| 3 | 614963b | test(03-02): add search pipeline integration tests with MockEmbedder |

## Known Stubs

None -- all functionality is fully wired. The search command produces real output from the SearchEngine.

## Self-Check: PASSED
