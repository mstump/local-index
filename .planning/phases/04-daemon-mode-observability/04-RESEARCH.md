# Phase 4: Daemon Mode & Observability - Research

**Researched:** 2026-04-10
**Domain:** File watching, metrics/observability, async concurrency, graceful shutdown
**Confidence:** HIGH

## Summary

Phase 4 transforms local-index from a batch CLI tool into a persistent daemon. Three subsystems must run concurrently in a single tokio runtime: (1) a file watcher that detects create/modify/rename/delete events and feeds them into the existing indexing pipeline, (2) an axum HTTP server exposing a Prometheus `/metrics` endpoint, and (3) a graceful shutdown coordinator that drains in-flight work on SIGINT/SIGTERM. The `status` CLI command queries the LanceDB store for aggregate statistics without starting the daemon.

The Rust ecosystem has mature, well-maintained crates for all three concerns. `notify` 8.x + `notify-debouncer-full` 0.7 handles cross-platform file watching with event coalescing. The `metrics` 0.24 facade + `metrics-exporter-prometheus` 0.18 provides zero-boilerplate Prometheus export. `tokio-util` 0.7 provides `CancellationToken` and `TaskTracker` for structured graceful shutdown. All integrate cleanly with the existing tokio + axum stack.

**Primary recommendation:** Use `CancellationToken` (not broadcast channel) for shutdown coordination -- it is purpose-built, cloneable, and integrates with `tokio::select!`. Use the `metrics` facade macros throughout the codebase; the Prometheus exporter attaches as a global recorder. The file watcher callback sends events over an `mpsc` channel to an async processing task that reuses the existing `ChunkStore` + `Embedder` pipeline.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-02 | `local-index daemon <path>` starts persistent watcher + HTTP server | notify-debouncer-full for watching, axum for HTTP, CancellationToken for lifecycle |
| CLI-04 | `local-index status` shows total chunks, files, last index time, pending queue depth, stale file count | LanceDB aggregate queries on existing ChunkStore; no daemon needed |
| WTCH-01 | Daemon uses `notify` with debounce for recursive create/modify/rename/delete watching | notify 8.2 + notify-debouncer-full 0.7; DebouncedEvent handles all event types |
| WTCH-02 | File rename = delete-old-path + index-new-path | DebouncedEvent provides rename tracking; process as delete + create pair |
| WTCH-03 | File delete removes all chunks for that file | Existing `ChunkStore::delete_chunks_for_file` already implements this |
| WTCH-04 | Watcher, embedder, and HTTP server run concurrently; graceful shutdown via broadcast on SIGINT/SIGTERM | CancellationToken + TaskTracker from tokio-util; tokio::signal for signal capture |
| OBS-01 | `/metrics` endpoint serves Prometheus-compatible output | metrics-exporter-prometheus + axum route calling `PrometheusHandle::render()` |
| OBS-02 | HDR histograms for embedding latency, indexing throughput, search latency, HTTP latency | `metrics::histogram!` macros with exponential bucket configuration via PrometheusBuilder |
| OBS-03 | Counters for total chunks indexed, embedding errors, file events processed, search queries served | `metrics::counter!` macros at instrumentation points |
| OBS-04 | Gauges for queue depth, total chunks, total files, stale file count | `metrics::gauge!` macros updated on state changes |
</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| notify | 8.2.0 | Cross-platform file watching | Standard Rust file watcher; FSEvents on macOS, inotify on Linux [VERIFIED: cargo search, docs.rs] |
| notify-debouncer-full | 0.7.0 | Event debouncing + rename tracking | Coalesces rapid events, tracks renames, deduplicates creates; depends on notify ^8.2 [VERIFIED: cargo search, docs.rs] |
| metrics | 0.24.3 | Metrics facade (counter/gauge/histogram macros) | Standard Rust metrics facade; decouples recording from export [VERIFIED: cargo search] |
| metrics-exporter-prometheus | 0.18.1 | Prometheus exposition format export | Official Prometheus backend for metrics facade; PrometheusHandle::render() for /metrics endpoint [VERIFIED: cargo search, docs.rs] |
| axum | 0.8.8 | HTTP server for /metrics endpoint | Already planned for Phase 5 web dashboard; needed now for metrics endpoint [VERIFIED: cargo search] |
| tower-http | 0.6.8 | HTTP middleware (tracing, timeout) | Standard companion to axum for middleware [VERIFIED: cargo search] |
| tokio-util | 0.7.18 | CancellationToken + TaskTracker | Official tokio utilities for graceful shutdown coordination [VERIFIED: cargo search] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tower | 0.5.3 | Middleware abstractions | Required by axum; use for timeout/rate-limit layers [VERIFIED: cargo search] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| CancellationToken | broadcast channel | Broadcast is lower-level; CancellationToken is purpose-built for shutdown, cloneable, integrates with select! |
| notify-debouncer-full | raw notify | Raw notify fires duplicate events, no rename tracking; debouncer handles all edge cases |
| metrics facade | prometheus crate (tikv) | Older API, verbose Registry management; metrics facade is more ergonomic |
| axum (single server) | Two separate servers | One server on one port is simpler for a local daemon; metrics is just another route |

**Installation (additions to Cargo.toml):**
```toml
# File watching
notify = "8.2"
notify-debouncer-full = "0.7"

# HTTP server (also used in Phase 5)
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["trace"] }

# Metrics
metrics = "0.24"
metrics-exporter-prometheus = "0.18"

# Shutdown coordination
tokio-util = "0.7"
```

## Architecture Patterns

### Recommended Project Structure
```
src/
  daemon/
    mod.rs           # DaemonRunner: orchestrates watcher + HTTP + shutdown
    watcher.rs       # FileWatcher: notify setup, event-to-channel bridge
    processor.rs     # EventProcessor: receives events, calls pipeline
    metrics.rs       # Metric names, recording helpers, PrometheusHandle setup
    http.rs          # Axum router: /metrics endpoint, /health endpoint
    shutdown.rs      # Signal handler + CancellationToken setup
  pipeline/          # (existing) chunker, embedder, store, walker
  search/            # (existing) engine, formatter, types
  cli.rs             # (existing) add daemon + status command handlers
  main.rs            # (existing) wire new daemon command
```

### Pattern 1: Event Channel Bridge (Watcher -> Processor)

**What:** The notify callback runs on a background thread (not async). Bridge to async tokio world via a bounded `mpsc::channel`.
**When to use:** Always -- notify's callback is sync; the indexing pipeline is async.

```rust
// Source: notify-debouncer-full docs + tokio patterns [VERIFIED: docs.rs]
use notify_debouncer_full::{new_debouncer, DebounceEventResult, DebouncedEvent};
use tokio::sync::mpsc;
use std::time::Duration;

// In daemon setup:
let (tx, mut rx) = mpsc::channel::<Vec<DebouncedEvent>>(100);

let mut debouncer = new_debouncer(
    Duration::from_millis(500),  // debounce window
    None,                         // no file ID cache (use default)
    move |result: DebounceEventResult| {
        if let Ok(events) = result {
            // tx.blocking_send because callback is sync
            let _ = tx.blocking_send(events);
        }
    },
)?;
debouncer.watch(&vault_path, notify::RecursiveMode::Recursive)?;

// Async processing loop:
tokio::spawn(async move {
    while let Some(events) = rx.recv().await {
        for event in events {
            process_event(event).await;
        }
    }
});
```

### Pattern 2: CancellationToken Shutdown Coordination

**What:** A single CancellationToken cloned to all tasks; signal handler cancels it; tasks use `select!` to detect cancellation.
**When to use:** WTCH-04 requires graceful shutdown on SIGINT/SIGTERM.

```rust
// Source: tokio.rs/topics/shutdown [CITED: tokio.rs/tokio/topics/shutdown]
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

let token = CancellationToken::new();
let tracker = TaskTracker::new();

// Signal handler
let shutdown_token = token.clone();
tokio::spawn(async move {
    tokio::signal::ctrl_c().await.ok();
    tracing::info!("shutdown signal received");
    shutdown_token.cancel();
});

// Watcher processing task
let watcher_token = token.clone();
tracker.spawn(async move {
    loop {
        tokio::select! {
            _ = watcher_token.cancelled() => break,
            Some(events) = rx.recv() => {
                process_events(events).await;
            }
        }
    }
    tracing::info!("watcher processor shut down");
});

// HTTP server with graceful shutdown
let http_token = token.clone();
tracker.spawn(async move {
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { http_token.cancelled().await })
        .await
});

// Wait for all tasks
tracker.close();
tracker.wait().await;
```

### Pattern 3: Metrics Instrumentation Points

**What:** Use the `metrics` facade macros at specific code locations; the global recorder (Prometheus exporter) collects them.
**When to use:** OBS-02/03/04 -- instrument the existing pipeline code.

```rust
// Source: metrics crate docs + axum prometheus example
// [VERIFIED: docs.rs/metrics/0.24, github.com/tokio-rs/axum/examples/prometheus-metrics]

// Setup (once, at daemon startup):
use metrics_exporter_prometheus::PrometheusBuilder;

let handle = PrometheusBuilder::new()
    .set_buckets_for_metric(
        metrics_exporter_prometheus::Matcher::Full("embedding_latency_seconds".to_string()),
        &[0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
    )?
    .install_recorder()?;

// Recording (throughout codebase):
metrics::counter!("chunks_indexed_total").increment(chunk_count as u64);
metrics::counter!("embedding_errors_total").increment(1);
metrics::counter!("file_events_processed_total").increment(1);
metrics::counter!("search_queries_total").increment(1);
metrics::gauge!("queue_depth").set(pending_count as f64);
metrics::gauge!("chunks_total").set(total_chunks as f64);
metrics::gauge!("files_total").set(total_files as f64);
metrics::gauge!("stale_files_total").set(stale_count as f64);
metrics::histogram!("embedding_latency_seconds").record(duration.as_secs_f64());
metrics::histogram!("indexing_throughput_chunks_per_second").record(throughput);
metrics::histogram!("search_latency_seconds").record(duration.as_secs_f64());
metrics::histogram!("http_request_duration_seconds").record(duration.as_secs_f64());

// Axum /metrics route:
let metrics_handle = handle.clone();
let app = Router::new()
    .route("/metrics", get(move || std::future::ready(metrics_handle.render())));
```

### Pattern 4: Status Command (Offline Query)

**What:** `local-index status` queries LanceDB directly without starting the daemon. Counts rows, distinct file paths, checks for stale files.
**When to use:** CLI-04.

```rust
// Pseudo-code for status command [ASSUMED: exact LanceDB query syntax]
// Count total chunks
let total_chunks = store.table().count_rows(None).await?;

// Count distinct files (query all file_path, deduplicate in Rust)
let file_paths = store.query_distinct_files().await?;
let total_files = file_paths.len();

// Queue depth is 0 when daemon is not running
// Stale file count: files in index whose disk mtime > stored indexed_at
```

### Anti-Patterns to Avoid
- **Spawning a thread per file event:** Use a bounded channel + single processing task. The embedder is the bottleneck (API calls), not event dispatch.
- **Using broadcast channel for shutdown:** `CancellationToken` is simpler, purpose-built, and integrates with `select!`. Broadcast requires receivers to handle `RecvError::Lagged`.
- **Blocking the notify callback:** The callback runs on notify's internal thread. Do NOT call async functions or perform I/O there. Use `blocking_send` to bridge to async.
- **Running metrics on a separate port:** For a local single-binary daemon, one axum server on one port with `/metrics` as a route is simpler than two servers.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| File event debouncing | Custom timer + event coalescing | notify-debouncer-full | Rename tracking, create dedup, timing edge cases are subtle |
| Prometheus exposition format | String formatting of metrics | metrics-exporter-prometheus | Format spec has gotchas (escaping, type headers, histogram encoding) |
| Graceful shutdown coordination | Manual Arc<AtomicBool> + Condvar | CancellationToken + TaskTracker | Race conditions, missed signals, incomplete drain are easy to get wrong |
| HDR histograms | Custom bucket math | metrics-exporter-prometheus bucket config | Bucket boundaries, overflow handling, and Prometheus encoding are non-trivial |
| Signal handling | Raw libc signal handlers | tokio::signal::ctrl_c() | Cross-platform, async-safe, no UB risk |

**Key insight:** The daemon's complexity is in orchestration (starting/stopping/coordinating tasks), not in any single algorithm. Every orchestration primitive has a battle-tested crate.

## Common Pitfalls

### Pitfall 1: Notify Callback Blocking
**What goes wrong:** Calling `async` functions or performing I/O in the notify debouncer callback causes deadlocks or dropped events.
**Why it happens:** The callback executes on notify's internal thread, which is not a tokio thread. Calling `.await` panics; blocking I/O stalls event delivery.
**How to avoid:** Use `tokio::sync::mpsc::Sender::blocking_send()` in the callback to bridge into the async world. Process events in a separate `tokio::spawn` task.
**Warning signs:** Deadlock on startup, missed file events, panic at `block_on` in non-async context.

### Pitfall 2: Debounce Window Too Short or Too Long
**What goes wrong:** Too short (< 100ms) = still get duplicate events on some platforms. Too long (> 2s) = noticeable delay before re-indexing.
**Why it happens:** macOS FSEvents batches events differently than Linux inotify. Obsidian saves files with a write-then-rename pattern.
**How to avoid:** Start with 500ms debounce window. This is fast enough for interactive use while absorbing platform-specific event bursts.
**Warning signs:** Duplicate re-indexes of the same file, or complaints about slow re-indexing.

### Pitfall 3: Forgetting to Update FTS Index After Daemon Re-Index
**What goes wrong:** Daemon re-indexes a file (updates LanceDB vectors) but the FTS index becomes stale because `ensure_fts_index()` is not called.
**Why it happens:** Phase 3 created FTS index at end of batch index. Daemon needs to rebuild FTS after each batch of changes.
**How to avoid:** After processing a batch of file events, call `ensure_fts_index()`. Consider batching: collect events for N seconds, process all, rebuild FTS once.
**Warning signs:** Full-text search returns stale results; semantic search is current but FTS lags behind.

### Pitfall 4: Metrics Recorder Not Installed Before Recording
**What goes wrong:** `metrics::counter!()` calls silently no-op if no global recorder is installed.
**Why it happens:** `PrometheusBuilder::install_recorder()` must be called before any metric macro is used. If daemon startup order is wrong, metrics are lost.
**How to avoid:** Install the Prometheus recorder as the very first thing in daemon startup, before spawning any tasks.
**Warning signs:** `/metrics` endpoint returns empty output despite activity.

### Pitfall 5: Status Command Assumes Daemon Is Running
**What goes wrong:** Status command tries to connect to daemon HTTP endpoint instead of reading LanceDB directly.
**Why it happens:** Natural design impulse to query the running daemon for "live" status.
**How to avoid:** Status command opens LanceDB directly and computes stats from stored data. "Pending queue depth" is 0 when daemon is not running (or N/A). This makes `status` work without daemon running.
**Warning signs:** `local-index status` fails when daemon is not running.

### Pitfall 6: File Rename Not Detected as Delete+Create
**What goes wrong:** Renamed file retains old file_path chunks in the index, and new path creates duplicate chunks.
**Why it happens:** `notify-debouncer-full` provides rename events with old and new paths, but if the handler only processes the new path, old path chunks remain.
**How to avoid:** For rename events: (1) delete all chunks for old path, (2) re-index the new path. The debouncer's `DebouncedEvent` provides both paths when available.
**Warning signs:** Chunk count grows unexpectedly; search returns results from files that have been renamed.

## Code Examples

### Daemon Main Loop Structure
```rust
// Source: Tokio shutdown docs + project architecture [CITED: tokio.rs/tokio/topics/shutdown]
pub async fn run_daemon(
    vault_path: PathBuf,
    bind_addr: String,
    data_dir: String,
) -> anyhow::Result<()> {
    // 1. Install metrics recorder FIRST
    let prom_handle = setup_metrics()?;

    // 2. Open store and create embedder
    let store = ChunkStore::open(&data_dir).await?;
    let api_key = resolve_voyage_key()?;
    let embedder = VoyageEmbedder::new(api_key);

    // 3. Set up shutdown coordination
    let token = CancellationToken::new();
    let tracker = TaskTracker::new();

    // 4. Set up file watcher with channel bridge
    let (event_tx, event_rx) = mpsc::channel(256);
    let debouncer = setup_watcher(&vault_path, event_tx)?;

    // 5. Spawn event processor
    tracker.spawn(event_processor(event_rx, store, embedder, token.clone()));

    // 6. Spawn HTTP server
    tracker.spawn(http_server(bind_addr, prom_handle, token.clone()));

    // 7. Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    tracing::info!("shutdown signal received, draining in-flight work...");
    token.cancel();

    // 8. Wait for all tasks to finish
    tracker.close();
    tracker.wait().await;
    tracing::info!("shutdown complete");

    // 9. Drop debouncer (stops notify thread)
    drop(debouncer);

    Ok(())
}
```

### Prometheus Handle with Axum
```rust
// Source: axum prometheus-metrics example
// [CITED: github.com/tokio-rs/axum/blob/main/examples/prometheus-metrics/src/main.rs]
use axum::{Router, routing::get};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

fn setup_metrics() -> anyhow::Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("embedding_latency_seconds".to_string()),
            &[0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        )?
        .set_buckets_for_metric(
            Matcher::Full("http_request_duration_seconds".to_string()),
            &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0],
        )?
        .install_recorder()?;
    Ok(handle)
}

fn metrics_router(handle: PrometheusHandle) -> Router {
    Router::new()
        .route("/metrics", get(move || std::future::ready(handle.render())))
        .route("/health", get(|| async { "ok" }))
}
```

### File Event Processing
```rust
// Source: project architecture + notify-debouncer-full docs [VERIFIED: docs.rs]
use notify_debouncer_full::DebouncedEvent;
use notify::EventKind;

async fn handle_event(
    event: &DebouncedEvent,
    vault_path: &Path,
    store: &ChunkStore,
    embedder: &impl Embedder,
) {
    let paths: Vec<&Path> = event.paths.iter().map(|p| p.as_path()).collect();

    match &event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            // Re-index the file
            for path in &paths {
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    metrics::counter!("file_events_processed_total").increment(1);
                    reindex_file(path, vault_path, store, embedder).await;
                }
            }
        }
        EventKind::Remove(_) => {
            // Delete all chunks for this file
            for path in &paths {
                let relative = path.strip_prefix(vault_path).unwrap_or(path);
                let relative_str = relative.to_string_lossy();
                if let Err(e) = store.delete_chunks_for_file(&relative_str).await {
                    tracing::warn!(error = %e, path = %relative_str, "failed to delete chunks");
                    metrics::counter!("embedding_errors_total").increment(1);
                }
                metrics::counter!("file_events_processed_total").increment(1);
            }
        }
        _ => {
            tracing::debug!(kind = ?event.kind, "ignoring event");
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| notify 6.x raw events | notify 8.x + debouncer-full 0.7 | 2024 | Rename tracking, event coalescing built-in |
| prometheus crate (tikv) | metrics facade + exporter | 2023 | Cleaner API, facade pattern, better ergonomics |
| Manual Arc<AtomicBool> shutdown | CancellationToken + TaskTracker | tokio-util 0.7 (2024) | Structured shutdown, no race conditions |
| broadcast channel for shutdown | CancellationToken | tokio guidance 2024+ | Purpose-built, no lagged receiver issues |

**Deprecated/outdated:**
- `notify` 6.x/7.x: CLAUDE.md mentions `^7.0` but current stable is 8.2.0; debouncer-full 0.7 requires ^8.2 [VERIFIED: docs.rs]
- `hotwatch`: Thin wrapper, lags behind notify [per CLAUDE.md]
- `prometheus` crate (tikv): Verbose, older pattern [per CLAUDE.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Status command can compute "stale file count" by comparing file mtimes against last-indexed timestamps stored in LanceDB | Pattern 4 / CLI-04 | LanceDB schema currently has no `indexed_at` timestamp column -- may need schema migration or a separate metadata store |
| A2 | 500ms debounce window is appropriate for Obsidian vaults | Pitfall 2 | May need tuning; could expose as `--debounce-ms` CLI flag |
| A3 | Single axum server for both /metrics and future Phase 5 dashboard is the right architecture | Pattern notes | Phase 5 may want different middleware or separate lifecycle |
| A4 | FTS index can be rebuilt after each batch of daemon events without excessive overhead | Pitfall 3 | For very frequent saves, FTS rebuild cost may be noticeable; may need throttling |
| A5 | `DebouncedEvent` from notify-debouncer-full provides both old and new paths for rename events | Pitfall 6 / WTCH-02 | Need to verify exact rename event structure in debouncer 0.7 |

## Open Questions (RESOLVED)

1. **Schema for `indexed_at` timestamp** — RESOLVED: Deferred. Plan 04-02 shows `last_index_time` as `null`/`unknown` since LanceDB schema has no `indexed_at` column yet. Adding the metadata table is deferred to a future phase when stale file detection is required.

2. **Pending queue depth when daemon is not running** — RESOLVED: Show `0` with a `(daemon not running)` note. Plan 04-02 Task 2 status command outputs `0` for queue depth when run standalone.

3. **Batch vs. immediate processing of file events** — RESOLVED: Process events per channel `recv_many` batch as received from the debouncer. Plan 04-03 Task 1 uses `channel.recv_many()` to collect available events and processes them together, amortizing FTS index rebuilds.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/cargo | Build | Checking... | See below | -- |
| tokio | Async runtime | Already in Cargo.toml | 1.40 | -- |

Note: All new dependencies are Rust crates added to Cargo.toml -- no external tools or services required beyond what is already in use.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + tokio::test for async |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-02 | `daemon` command starts and shuts down cleanly | integration | `cargo test --test daemon_integration` | No -- Wave 0 |
| CLI-04 | `status` command returns correct stats | integration | `cargo test --test status_integration` | No -- Wave 0 |
| WTCH-01 | File create/modify events trigger re-index | integration | `cargo test --test daemon_integration::test_file_create` | No -- Wave 0 |
| WTCH-02 | File rename deletes old path chunks + indexes new path | integration | `cargo test --test daemon_integration::test_file_rename` | No -- Wave 0 |
| WTCH-03 | File delete removes chunks | integration | `cargo test --test daemon_integration::test_file_delete` | No -- Wave 0 |
| WTCH-04 | SIGINT drains in-flight work | integration | `cargo test --test daemon_integration::test_graceful_shutdown` | No -- Wave 0 |
| OBS-01 | /metrics returns Prometheus format | unit | `cargo test --lib daemon::http::test_metrics_endpoint` | No -- Wave 0 |
| OBS-02 | Histograms recorded with correct names | unit | `cargo test --lib daemon::metrics::test_histogram_names` | No -- Wave 0 |
| OBS-03 | Counters increment on events | unit | `cargo test --lib daemon::metrics::test_counters` | No -- Wave 0 |
| OBS-04 | Gauges reflect current state | unit | `cargo test --lib daemon::metrics::test_gauges` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `tests/daemon_integration.rs` -- integration tests for daemon lifecycle, file events, shutdown
- [ ] `tests/status_integration.rs` -- integration tests for status command output
- [ ] Unit tests within `src/daemon/` modules for metrics recording and event processing

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | Local daemon on 127.0.0.1; no auth per project requirements |
| V3 Session Management | No | No sessions |
| V4 Access Control | No | Single-user local daemon |
| V5 Input Validation | Yes | Validate file paths from notify events; sanitize SQL filter strings passed to LanceDB |
| V6 Cryptography | No | No crypto operations in this phase |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via crafted symlinks | Tampering | Canonicalize paths, ensure they are under vault_path before processing |
| Resource exhaustion from rapid file events | Denial of Service | Bounded mpsc channel (256); debounce window; backpressure via channel fullness |
| SQL injection in LanceDB filter strings | Tampering | Existing pattern uses format! with string values; ensure proper escaping (already uses `replace('\'', "''")`) |

## Sources

### Primary (HIGH confidence)
- [notify 8.2.0 docs](https://docs.rs/notify/8.2.0/) -- API, platform backends, event types
- [notify-debouncer-full 0.7.0 docs](https://docs.rs/notify-debouncer-full/0.7.0/) -- Debouncer API, dependency on notify ^8.2
- [metrics 0.24.3 docs](https://docs.rs/metrics/0.24.3/) -- Facade macros: counter!, gauge!, histogram!
- [metrics-exporter-prometheus 0.18.1 docs](https://docs.rs/metrics-exporter-prometheus/0.18.1/) -- PrometheusBuilder, PrometheusHandle, install_recorder
- [axum prometheus-metrics example](https://github.com/tokio-rs/axum/blob/main/examples/prometheus-metrics/src/main.rs) -- Integration pattern for /metrics endpoint
- [Tokio graceful shutdown guide](https://tokio.rs/tokio/topics/shutdown) -- CancellationToken, TaskTracker patterns
- cargo search (crates.io registry) -- version verification for all crates

### Secondary (MEDIUM confidence)
- [Exporting Prometheus metrics with Axum](https://ellie.wtf/notes/exporting-prometheus-metrics-with-axum) -- Community guide for axum + metrics integration

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all crates verified via cargo search and docs.rs; versions confirmed current
- Architecture: HIGH -- patterns follow official tokio/axum documentation and examples
- Pitfalls: HIGH -- based on verified API behavior and known platform differences

**Research date:** 2026-04-10
**Valid until:** 2026-05-10 (30 days -- stable ecosystem, no major releases expected)
