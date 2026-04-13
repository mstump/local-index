---
phase: 04-daemon-mode-observability
plan: 03
title: "File Watcher, Event Processor, and Daemon Wiring"
subsystem: daemon
tags: [daemon, file-watcher, notify, event-processor, shutdown, graceful-shutdown]
dependency_graph:
  requires: [metrics-recorder, http-router, health-endpoint, chunk-count-api, file-count-api]
  provides: [file-watcher, event-processor, daemon-runner, graceful-shutdown, daemon-cli-command]
  affects: [src/main.rs, src/daemon/mod.rs]
tech_stack:
  added: []
  patterns: [notify-debouncer-full with mpsc channel bridge, CancellationToken for graceful shutdown, TaskTracker for concurrent task coordination, vault path validation on file events]
key_files:
  created:
    - src/daemon/watcher.rs
    - src/daemon/processor.rs
    - src/daemon/shutdown.rs
    - tests/daemon_smoke.rs
  modified:
    - src/daemon/mod.rs
    - src/main.rs
    - Cargo.toml
decisions:
  - "tokio-util rt feature required for TaskTracker (was not enabled by Plan 01)"
  - "Debouncer<RecommendedWatcher, RecommendedCache> is correct type for notify-debouncer-full 0.5"
  - "DebouncedEvent derefs to notify::Event so .kind and .paths accessible directly"
  - "Vault path validation (T-04-05) added as strip_prefix check on all event paths before processing"
  - "Smoke test uses tower::ServiceExt::oneshot on router directly (no real daemon startup needed)"
metrics:
  duration: 16min
  completed: 2026-04-10
  tasks_completed: 2
  tasks_total: 2
  files_changed: 7
  tests_added: 1
  tests_total: 71
requirements:
  - CLI-02
  - WTCH-01
  - WTCH-02
  - WTCH-03
  - WTCH-04
---

# Phase 04 Plan 03: File Watcher, Event Processor, and Daemon Wiring Summary

File watcher using notify-debouncer-full bridged to tokio mpsc, event processor with explicit rename handling (Both/From/To modes per WTCH-02), CancellationToken-based graceful shutdown, and daemon orchestrator tying watcher + processor + HTTP server into a single `local-index daemon <path>` command.

## Task Results

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Implement watcher, processor, shutdown, and daemon orchestrator | de73cbc | src/daemon/watcher.rs, src/daemon/processor.rs, src/daemon/shutdown.rs, src/daemon/mod.rs, Cargo.toml |
| 2 | Wire daemon command in main.rs and add integration test | c683d53 | src/main.rs, tests/daemon_smoke.rs |

## Implementation Details

### Task 1: Daemon Subsystem Modules

Four new modules created under `src/daemon/`:

- **shutdown.rs**: Sets up a `CancellationToken` that is cancelled on first SIGINT/SIGTERM. A second signal forces immediate exit via `std::process::exit(1)`.

- **watcher.rs**: `FileWatcher` struct wrapping `notify_debouncer_full::Debouncer<RecommendedWatcher, RecommendedCache>`. Uses 500ms debounce window. Callback uses `blocking_send` on tokio mpsc channel (notify callback runs on sync thread). Bounded channel (256 capacity) provides backpressure (T-04-06 mitigation).

- **processor.rs**: `run_event_processor` loop receiving `Vec<DebouncedEvent>` batches. Handles all event types:
  - Create/Modify: re-index file (read, chunk, embed, delete old chunks, store new)
  - Remove: delete all chunks for file
  - Rename (WTCH-02): explicit `RenameMode::Both` (delete old + index new), `RenameMode::From` (delete old), `RenameMode::To` (index new)
  - All paths validated against vault_path via `strip_prefix` (T-04-05 mitigation)
  - After each batch: updates chunks_total and files_total gauges, records indexing throughput, rebuilds FTS index

- **mod.rs**: `run_daemon` orchestrator that sets up metrics recorder, opens ChunkStore, creates embedder, installs signal handler, starts FileWatcher, spawns event processor and HTTP server via `TaskTracker`, then awaits shutdown token cancellation.

Also enabled `rt` feature on `tokio-util` for `TaskTracker` support.

### Task 2: CLI Wiring and Smoke Test

Replaced the daemon command stub in `main.rs` with full wiring: canonicalizes vault path, resolves data directory (same pattern as index command), calls `run_daemon`.

Created `tests/daemon_smoke.rs` integration test that exercises the HTTP router directly via `tower::ServiceExt::oneshot` without requiring a real vault or API key.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] tokio-util missing rt feature for TaskTracker**
- **Found during:** Task 1
- **Issue:** `tokio_util::task::TaskTracker` requires the `rt` feature which was not enabled in Cargo.toml
- **Fix:** Changed `tokio-util = "0.7"` to `tokio-util = { version = "0.7", features = ["rt"] }`
- **Files modified:** Cargo.toml

**2. [Rule 2 - Security] Added vault path validation (T-04-05 mitigation)**
- **Found during:** Task 1
- **Issue:** Plan's threat model (T-04-05) specifies mitigating path traversal via symlinks by validating event paths are within vault
- **Fix:** Added `strip_prefix(&vault_path)` validation on all event paths before processing, with warning log for out-of-vault paths
- **Files modified:** src/daemon/processor.rs

## Verification

All verification criteria met:
1. `cargo check` passes -- all daemon modules compile
2. `cargo test --test daemon_smoke` passes (1 test) -- HTTP endpoints verified
3. `cargo build` passes -- binary links successfully
4. `cargo test --lib` passes (70 tests, 0 failures, no regressions)

## Self-Check: PASSED
