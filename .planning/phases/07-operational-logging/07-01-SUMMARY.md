---
phase: 07-operational-logging
plan: 01
subsystem: logging
tags: [logging, tracing, daemon, search, lancedb]
dependency_graph:
  requires: []
  provides: [structured-search-logging, structured-daemon-logging, lancedb-noise-suppression]
  affects: [src/search/engine.rs, src/web/handlers.rs, src/daemon/processor.rs, src/main.rs]
tech_stack:
  added: []
  patterns: [structured-tracing-fields, instant-timing, envfilter-defaults]
key_files:
  created: []
  modified:
    - src/search/engine.rs
    - src/web/handlers.rs
    - src/daemon/processor.rs
    - src/main.rs
decisions:
  - "Web handler logs 'web search completed' (distinct from engine-level 'search completed') to identify search origin"
  - "LanceDB suppression applied only in the fallback default; RUST_LOG override fully respected"
  - "chunks_removed hardcoded to 0 in reindex_file (delete-then-rewrite pattern; old chunk count not tracked)"
metrics:
  duration_seconds: 323
  completed_date: "2026-04-14"
  tasks_completed: 3
  files_modified: 4
requirements:
  - LOG-01
  - LOG-02
  - LOG-03
---

# Phase 07 Plan 01: Structured Operational Logging Summary

**One-liner:** Structured INFO logging for all search queries (CLI + web) with latency_ms timing, all daemon file events with event type and vault-relative path, indexing outcomes with chunks_added/removed/skipped, and lancedb=warn,lance=warn suppression in the default EnvFilter.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Search query logging with timing (LOG-01) | 8977b05 | src/search/engine.rs, src/web/handlers.rs |
| 2 | Daemon file event and indexing outcome logging (LOG-02) | 780d74d | src/daemon/processor.rs |
| 3 | Suppress LanceDB tracing noise (LOG-03) | 5311ba8 | src/main.rs |

## What Was Implemented

### LOG-01: Search Query Logging

`SearchEngine::search()` now records a `std::time::Instant` at entry and emits a structured INFO log before returning:

```
query=<text> mode=<semantic|fts|hybrid> results_returned=<n> latency_ms=<ms> message="search completed"
```

`search_handler()` (web) emits the same fields with message `"web search completed"`, allowing operators to distinguish CLI vs. web search origin in logs.

### LOG-02: Daemon File Event Logging

Every daemon file event now produces an INFO `"file event"` log line before processing, with structured fields:
- `event` = Created | Modified | Renamed | Deleted
- `path` = vault-relative path
- `renamed_to` = vault-relative new path (rename-both arm only)

After processing, every outcome produces an INFO `"indexing outcome"` log with:
- `path` = vault-relative path
- `chunks_added`, `chunks_removed`, `chunks_skipped`

The `file` field name in `remove_file()` was normalized to `path` for consistency.

### LOG-03: LanceDB Noise Suppression

The `init_logging()` fallback EnvFilter now includes `lancedb=warn,lance=warn`:

```rust
EnvFilter::new(format!("{},lancedb=warn,lance=warn", log_level))
```

When `RUST_LOG` is set, the user's value takes full precedence (no change to behavior).

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None introduced by this plan.

## Threat Flags

The threat model identified T-07-02 (log injection via structured fields). The `tracing::info!(query = %opts.query, ...)` syntax uses structured fields, not format string interpolation, so tracing handles escaping of control characters automatically. No additional mitigations needed.

## Self-Check: PASSED

All four modified files exist. All three task commits verified in git log (8977b05, 780d74d, 5311ba8). cargo check and cargo test both pass.
