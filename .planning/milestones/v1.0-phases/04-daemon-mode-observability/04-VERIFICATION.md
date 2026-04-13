---
phase: 04-daemon-mode-observability
verified: 2026-04-10T23:56:20Z
status: gaps_found
score: 4/5 must-haves verified
overrides_applied: 0
gaps:
  - truth: "SIGINT/SIGTERM triggers graceful shutdown that completes in-flight work"
    status: partial
    reason: "shutdown.rs uses tokio::signal::ctrl_c() which handles SIGINT only on Unix. SIGTERM is not handled. The docstring says 'SIGINT/SIGTERM' but the implementation catches only ctrl_c. On Linux/macOS, kill <pid> sends SIGTERM by default and the daemon will not shut down gracefully."
    artifacts:
      - path: "src/daemon/shutdown.rs"
        issue: "Only tokio::signal::ctrl_c() is called; no tokio::signal::unix::signal(SignalKind::terminate()) handler is registered"
    missing:
      - "Add SIGTERM handler using tokio::signal::unix (Unix) or tokio::signal::windows (Windows) alongside the ctrl_c handler"
      - "Use tokio::select! to race ctrl_c() against SIGTERM signal stream, cancelling the token on whichever fires first"
deferred:
  - truth: "status command shows last index time with a real timestamp value"
    addressed_in: "Phase 5"
    evidence: "Phase 5 SC3: 'Dashboard index browser lists all indexed files with per-file chunk count and last-indexed timestamp'. The plan 04-02 decision explicitly notes: 'last_index_time is null/unknown (no indexed_at column yet) -- accurate representation, not placeholder'. The schema has no indexed_at column; adding per-file timestamps is planned for Phase 5."
human_verification:
  - test: "Manual daemon file-watch loop"
    expected: "Running 'local-index daemon <path>' and creating/modifying/deleting a .md file produces re-indexing log lines and the index is updated"
    why_human: "Cannot test notify file-system events or actual embedding API calls programmatically in this context. Requires a real vault path, VOYAGE_API_KEY, and observing tracing output."
  - test: "SIGINT graceful shutdown"
    expected: "Pressing Ctrl-C once logs 'shutdown signal received, draining in-flight work...' and the daemon exits cleanly with 'daemon shutdown complete'"
    why_human: "Interactive signal delivery cannot be automated in a static verification check"
---

# Phase 4: Daemon Mode & Observability Verification Report

**Phase Goal:** Operator can run a persistent daemon that watches for file changes and re-indexes in real time, with full Prometheus metrics
**Verified:** 2026-04-10T23:56:20Z
**Status:** gaps_found
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Operator can run `local-index daemon <path>` and process watches for file changes | VERIFIED | `src/daemon/watcher.rs` FileWatcher using notify-debouncer-full (500ms debounce); `run_daemon` in `src/daemon/mod.rs` wired to CLI in `src/main.rs` line 356 |
| 2 | File renames handled as delete-old + index-new; file deletes remove all chunks | VERIFIED | `src/daemon/processor.rs` handles RenameMode::Both/From/To explicitly (lines 72-139); EventKind::Remove handled (lines 153-158) |
| 3 | `local-index status` shows total chunks, files, last index time, queue depth, stale file count | VERIFIED (partial) | `src/main.rs` lines 430-479 fully implemented; `count_total_chunks`, `count_distinct_files` wired; queue_depth=0 with "daemon not running" note; last_index_time=null (no schema column -- see Deferred Items) |
| 4 | /metrics endpoint serves Prometheus metrics with HDR histograms for embedding/indexing/search/HTTP latency | VERIFIED | `src/daemon/metrics.rs` has 4 histogram metric constants with custom buckets; `src/daemon/http.rs` serves /metrics via axum; smoke test passes |
| 5 | Graceful shutdown on SIGINT/SIGTERM completes in-flight work | PARTIAL | `src/daemon/shutdown.rs` handles SIGINT via `tokio::signal::ctrl_c()` but has no SIGTERM handler; docstring claims SIGTERM support but implementation does not provide it |

**Score:** 4/5 truths verified

### Deferred Items

Items not yet met but explicitly addressed in later milestone phases.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | Status command last_index_time shows a real timestamp | Phase 5 | Phase 5 SC3: "Dashboard index browser lists all indexed files with per-file chunk count and last-indexed timestamp"; plan 04-02 decision notes null is accurate given no indexed_at schema column |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | All Phase 4 dependency crates | VERIFIED | notify-debouncer-full=0.5, axum=0.8, metrics=0.24, metrics-exporter-prometheus=0.16, tokio-util=0.7+rt feature, chrono=0.4 |
| `src/daemon/mod.rs` | Module declarations + run_daemon | VERIFIED | All 5 submodules declared; `pub async fn run_daemon` present; TaskTracker, axum::serve, graceful shutdown wired |
| `src/daemon/metrics.rs` | Prometheus setup, 12 metric constants, recording helpers | VERIFIED | 4 counter + 4 gauge + 4 histogram constants; setup_metrics() with custom buckets for all 4 histograms; 12 convenience functions |
| `src/daemon/http.rs` | Axum router with /metrics and /health | VERIFIED | metrics_router() returns Router with both routes; /health returns "ok"; /metrics renders Prometheus text |
| `src/daemon/watcher.rs` | FileWatcher wrapping notify-debouncer-full | VERIFIED | FileWatcher struct with new_debouncer, blocking_send bridge to mpsc |
| `src/daemon/processor.rs` | Event processor calling indexing pipeline | VERIFIED | run_event_processor; handles Create/Modify/Remove/Rename (Both+From+To); vault path validation; FTS rebuild; metrics recording |
| `src/daemon/shutdown.rs` | Signal handler + CancellationToken | PARTIAL | CancellationToken present; ctrl_c handler works for SIGINT; SIGTERM not handled |
| `src/pipeline/store.rs` | count_total_chunks, count_distinct_files, get_all_file_paths | VERIFIED | All 3 methods present at lines 296, 304, 310; 17 store tests pass |
| `src/main.rs` | Status command + daemon command wired | VERIFIED | Status command fully implemented (lines 430-479); daemon calls run_daemon (line 356); no stubs |
| `tests/daemon_smoke.rs` | Integration test for HTTP endpoints | VERIFIED | test_daemon_health_and_metrics_endpoints tests /health and /metrics via tower oneshot |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/daemon/http.rs` | `src/daemon/metrics.rs` | PrometheusHandle.render() in /metrics handler | VERIFIED | Arc<PrometheusHandle> passed to route closure; handle.render() called on line 16 |
| `src/main.rs` | `src/pipeline/store.rs` | ChunkStore::open then count_total_chunks, count_distinct_files | VERIFIED | Lines 453-456 in main.rs; store.count_total_chunks().await and store.count_distinct_files().await both called |
| `src/daemon/watcher.rs` | `src/daemon/processor.rs` | mpsc channel sending Vec<DebouncedEvent> | VERIFIED | mpsc::channel(256) in mod.rs; FileWatcher::new receives tx; run_event_processor receives rx |
| `src/daemon/processor.rs` | `src/pipeline/store.rs` | ChunkStore::delete_chunks_for_file and store_chunks | VERIFIED | store.delete_chunks_for_file called in reindex_file (line 245) and remove_file (line 274) |
| `src/daemon/shutdown.rs` | `src/daemon/mod.rs` | CancellationToken shared with all tasks | VERIFIED | token from setup_shutdown() cloned for proc_token and http_token; token.cancelled().await in run_daemon |
| `src/main.rs` | `src/daemon/mod.rs` | run_daemon() called from CLI dispatch | VERIFIED | local_index::daemon::run_daemon(vault_path, bind.clone(), db_path).await? at line 356 |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `src/daemon/http.rs` /metrics handler | rendered (String from handle.render()) | PrometheusHandle from setup_metrics() global recorder | Yes -- recorder accumulates live metric recordings | FLOWING |
| `src/daemon/processor.rs` reindex_file | result.embeddings (Vec<Vec<f32>>) | embedder.embed(&texts).await -- real API call | Yes -- calls Voyage API | FLOWING |
| `src/main.rs` Status command | total_chunks, total_files | store.count_total_chunks(), store.count_distinct_files() -- real LanceDB queries | Yes -- queries LanceDB table | FLOWING |
| `src/main.rs` Status command | last_index_time | Hardcoded null/unknown | No -- no indexed_at schema column | STATIC (documented limitation, deferred) |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| metrics library tests pass | cargo test daemon::metrics --lib | 3 passed, 0 failed | PASS |
| HTTP library tests pass | cargo test daemon::http --lib | 2 passed, 0 failed | PASS |
| daemon smoke integration test passes | cargo test --test daemon_smoke | 1 passed, 0 failed | PASS |
| store aggregate tests pass | cargo test store::tests --lib | 17 passed, 0 failed | PASS |
| project compiles | cargo check | Finished dev profile, 0 errors | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| OBS-01 | 04-01 | Prometheus metrics recorder installed globally | SATISFIED | setup_metrics() installs recorder; PrometheusHandle returned |
| OBS-02 | 04-01 | HDR histograms for latency-sensitive operations | SATISFIED | Custom buckets for embedding_latency_seconds, search_latency_seconds, http_request_duration_seconds, indexing_throughput_chunks_per_second |
| OBS-03 | 04-01 | Counter metrics defined and recorded | SATISFIED | 4 counter constants; increment_chunks_indexed, increment_embedding_errors, increment_file_events, increment_search_queries |
| OBS-04 | 04-01 | Gauge metrics defined and recorded | SATISFIED | 4 gauge constants; queue_depth, chunks_total, files_total, stale_files_total |
| CLI-04 | 04-02 | `local-index status` command | SATISFIED | Status command fully implemented; TTY table + JSON pipe output |
| CLI-02 | 04-03 | `local-index daemon <path>` command | SATISFIED | Daemon command wired; calls run_daemon |
| WTCH-01 | 04-03 | File create events trigger re-indexing | SATISFIED | EventKind::Create matched in processor.rs line 144 |
| WTCH-02 | 04-03 | File rename events: delete-old + index-new | SATISFIED | Explicit RenameMode::Both/From/To handling in processor.rs lines 72-139 |
| WTCH-03 | 04-03 | File modify events trigger re-indexing | SATISFIED | EventKind::Modify matched in processor.rs line 144 |
| WTCH-04 | 04-03 | File delete events remove chunks | SATISFIED | EventKind::Remove matched in processor.rs line 153 |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/daemon/shutdown.rs` | 10 | SIGTERM not handled; only ctrl_c() registered | Blocker | On Linux/macOS, process managers (systemd, launchd, docker) send SIGTERM to stop services; the daemon will not shut down gracefully when deployed with these tools |
| `src/main.rs` | 471 | `"last_index_time": null` hardcoded | Warning | Feature gap acknowledged in plan; no indexed_at column in schema; deferred to Phase 5 |

### Human Verification Required

#### 1. File watcher end-to-end loop

**Test:** Run `VOYAGE_API_KEY=<key> cargo run -- daemon <vault_path>`, then create a .md file in `<vault_path>`, modify it, rename it, and delete it.
**Expected:** Each operation produces tracing log lines showing "re-indexing file", "removing chunks for deleted file", or rename variant messages. After re-index, `local-index status` shows updated chunk/file counts.
**Why human:** Requires live file system events from notify, a real vault path, and the Voyage embedding API. Cannot be reproduced with static code analysis.

#### 2. SIGINT graceful shutdown

**Test:** Start the daemon, press Ctrl-C once, observe logs.
**Expected:** First Ctrl-C logs "shutdown signal received, draining in-flight work..." then the process completes in-flight work and logs "daemon shutdown complete". A second Ctrl-C forces immediate exit.
**Why human:** Interactive signal delivery requires a running process.

#### 3. SIGTERM graceful shutdown (KNOWN GAP)

**Test:** Start the daemon, run `kill <daemon_pid>` (which sends SIGTERM), observe behavior.
**Expected:** Daemon should shut down gracefully (based on roadmap SC5: "SIGINT/SIGTERM").
**Actual:** Daemon will NOT respond to SIGTERM. The process will continue running. Only `kill -SIGINT <pid>` or Ctrl-C will trigger graceful shutdown.
**Why human:** Requires running process and signal delivery.

### Gaps Summary

One gap blocks the phase goal from being fully satisfied:

**SIGTERM not handled (SC5 partial):** The roadmap success criterion 5 states "Graceful shutdown on SIGINT/SIGTERM." The implementation in `src/daemon/shutdown.rs` uses only `tokio::signal::ctrl_c()`, which handles SIGINT (Ctrl-C) but not SIGTERM. On Linux and macOS, process managers (systemd, launchd, Docker) send SIGTERM as the default stop signal. A daemon that does not handle SIGTERM will be forcibly killed (SIGKILL) after the system's grace period, losing in-flight indexing work. This is a direct violation of the roadmap SC.

The fix is straightforward: add a `tokio::signal::unix::signal(SignalKind::terminate())` handler in a `tokio::select!` alongside the ctrl_c handler. The plan's code example referenced SIGTERM in the docstring but the implementation did not include it.

The last_index_time limitation (always null) is explicitly deferred to Phase 5 and does not constitute a gap for Phase 4.

---

_Verified: 2026-04-10T23:56:20Z_
_Verifier: Claude (gsd-verifier)_
