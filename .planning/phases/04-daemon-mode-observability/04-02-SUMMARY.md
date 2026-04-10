---
phase: 04-daemon-mode-observability
plan: 02
title: "Status CLI Command"
subsystem: cli
tags: [cli, status, lancedb, query]
dependency_graph:
  requires: []
  provides: [status-command, chunk-count-api, file-count-api]
  affects: [src/pipeline/store.rs, src/main.rs]
tech_stack:
  added: []
  patterns: [aggregate-query-methods, tty-vs-json-output]
key_files:
  created: []
  modified:
    - src/pipeline/store.rs
    - src/main.rs
decisions:
  - "count_total_chunks uses LanceDB count_rows(None) for O(1) row count"
  - "count_distinct_files queries file_path column and deduplicates via HashSet"
  - "get_all_file_paths returns deduplicated Vec<String> for reuse by daemon stale-file detection"
  - "last_index_time is null/unknown (no indexed_at column yet) -- accurate representation, not placeholder"
  - "stale_files is 0 (no mtime tracking yet) -- will be populated when daemon adds per-file timestamps"
metrics:
  duration: "18min"
  completed: "2026-04-10T23:14:46Z"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
---

# Phase 04 Plan 02: Status CLI Command Summary

Aggregate query methods on ChunkStore plus a fully wired `local-index status` command with TTY table and JSON pipe output.

## Task Results

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add aggregate query methods to ChunkStore | 6dd3272 | src/pipeline/store.rs |
| 2 | Wire status command handler in main.rs | a8706e1 | src/main.rs |

## Implementation Details

### Task 1: Aggregate Query Methods (TDD)

Added three public methods to `ChunkStore`:

- `count_total_chunks()` -- uses `table.count_rows(None)` for efficient row count
- `count_distinct_files()` -- queries file_path column, deduplicates via HashSet
- `get_all_file_paths()` -- returns deduplicated file path list (reusable for daemon stale-file detection)

Four TDD tests added covering empty and populated stores for both count methods.

### Task 2: Status Command Handler

Replaced the status stub in main.rs with a full implementation:

- Resolves data directory using same pattern as search command
- Opens ChunkStore and queries aggregate counts
- TTY output: human-readable table (Total chunks, Total files, Last index time, Queue depth, Stale files, Data directory)
- Pipe output: JSON object with all fields
- Queue depth shows `0` with "daemon not running" note
- Graceful handling when no index directory exists

## Deviations from Plan

None -- plan executed exactly as written.

## Verification

- `cargo test store::tests --lib` -- 17 tests pass (including 4 new aggregate tests)
- `cargo build` -- compiles successfully
- `cargo test --lib` -- 65 tests pass (no regressions)

## Self-Check: PASSED
