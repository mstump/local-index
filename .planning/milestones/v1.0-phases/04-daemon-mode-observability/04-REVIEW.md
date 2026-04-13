---
phase: 04-daemon-mode-observability
reviewed: 2026-04-10T00:00:00Z
depth: standard
files_reviewed: 11
files_reviewed_list:
  - Cargo.toml
  - src/daemon/http.rs
  - src/daemon/metrics.rs
  - src/daemon/mod.rs
  - src/daemon/processor.rs
  - src/daemon/shutdown.rs
  - src/daemon/watcher.rs
  - src/lib.rs
  - src/main.rs
  - src/pipeline/store.rs
  - tests/daemon_smoke.rs
findings:
  critical: 2
  warning: 4
  info: 3
  total: 9
status: issues_found
---

# Phase 04: Code Review Report

**Reviewed:** 2026-04-10
**Depth:** standard
**Files Reviewed:** 11
**Status:** issues_found

## Summary

This phase implements daemon mode (file watcher + event processor + HTTP observability server) with Prometheus metrics. The overall architecture is sound: cancellation token propagation, graceful shutdown, task tracking, and debounced file watching are all correctly wired. The metrics module is clean and the HTTP router is minimal and correct.

Two critical issues require fixes before shipping: a SQL injection vulnerability in `store.rs` (user-controlled file paths interpolated into filter strings) and a double-increment of the `file_events_processed_total` counter in `processor.rs` for the `Remove` event path (metrics accounting is broken for deletes). Four warnings cover correctness risks: blocking I/O on the async executor in `reindex_file`, a channel back-pressure loss in the watcher, a misleading `remove_file` function that increments the file event counter internally AND is called by callers that also increment it (in some paths), and a missed embedder error classification (file-read failure increments `embedding_errors_total`, not a distinct read-error counter). Three info items are lower-priority quality notes.

---

## Critical Issues

### CR-01: SQL Injection via Unsanitized File Path in LanceDB Filter Strings

**File:** `src/pipeline/store.rs:201`, `src/pipeline/store.rs:234`

**Issue:** `get_hashes_for_file` and `delete_chunks_for_file` interpolate the caller-supplied `file_path` string directly into LanceDB SQL filter expressions using Rust `format!`:

```rust
// line 201
.only_if(format!("file_path = '{}'", file_path))

// line 234
self.table.delete(&format!("file_path = '{}'", file_path))
```

A file path containing a single quote (valid on Linux/macOS, e.g. `it's a note.md`) will break the filter expression and could, depending on LanceDB's SQL dialect, be exploited to match unintended rows or cause data corruption. On macOS/Linux, single-quoted filenames are unusual but entirely legal. In a vault-watching daemon that accepts arbitrary user files, this is a realistic path. At minimum it causes silent data loss (wrong rows deleted).

**Fix:** Use parameterized queries if LanceDB exposes them. If not, escape single quotes by doubling them before interpolation:

```rust
fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

// Then:
.only_if(format!("file_path = '{}'", escape_sql_string(file_path)))
self.table.delete(&format!("file_path = '{}'", escape_sql_string(file_path)))
```

Apply consistently everywhere a caller-supplied string is interpolated into a filter. Check whether LanceDB's Rust API offers a parameterized/prepared filter API as a longer-term fix.

---

### CR-02: Double-Increment of `file_events_processed_total` Counter for Remove Events

**File:** `src/daemon/processor.rs:153-157`, `src/daemon/processor.rs:278`

**Issue:** When `EventKind::Remove` is processed, the loop in `run_event_processor` calls `remove_file(path, &vault_path, &store).await` and then calls `metrics::increment_file_events()` (line 156). However, `remove_file` itself unconditionally calls `metrics::increment_file_events()` at its end (line 278). Every Remove event therefore increments the counter twice.

This contrasts with the Create/Modify path which does NOT call `increment_file_events` inside `reindex_file` — the caller handles it. The asymmetry causes the `file_events_processed_total` metric to be systematically inflated for delete events, corrupting all derived alerting and dashboards.

Reproduction: rename a file out of the vault (RenameMode::From) — `remove_file` is called, then `metrics::increment_file_events()` is also called inside `remove_file`, and the calling site at line 112 adds another increment for that code path too.

**Fix:** Remove the `metrics::increment_file_events()` call from inside `remove_file` (line 278) and let callers be responsible for incrementing, consistent with how `reindex_file` is structured:

```rust
// In remove_file: remove the final metrics call
async fn remove_file(path: &Path, vault_path: &Path, store: &ChunkStore) {
    let relative = path.strip_prefix(vault_path).unwrap_or(path);
    let relative_str = relative.to_string_lossy().to_string();
    tracing::info!(file = %relative_str, "removing chunks for deleted file");
    if let Err(e) = store.delete_chunks_for_file(&relative_str).await {
        tracing::warn!(file = %relative_str, error = %e, "failed to remove chunks for deleted file");
        metrics::increment_embedding_errors();
    }
    // DO NOT call metrics::increment_file_events() here -- callers handle it
}
```

---

## Warnings

### WR-01: Blocking File I/O on the Async Executor Thread

**File:** `src/daemon/processor.rs:203`

**Issue:** `reindex_file` calls `std::fs::read_to_string(path)` — a synchronous blocking call — directly on the tokio async executor thread. For large markdown files this blocks the entire executor thread for the duration of the read, starving other async tasks including the HTTP server and event loop. This is a correctness concern for daemon responsiveness: if a large file is written to the vault, the `/health` endpoint will become unresponsive until the read completes.

**Fix:** Replace the blocking read with `tokio::fs::read_to_string`:

```rust
// Before:
let content = match std::fs::read_to_string(path) {

// After:
let content = match tokio::fs::read_to_string(path).await {
```

`reindex_file` is already an `async fn`, so this requires no signature changes.

---

### WR-02: Watcher Channel Back-Pressure: Silent Event Drop on Full Channel

**File:** `src/daemon/watcher.rs:28`

**Issue:** The notify debouncer callback uses `tx.blocking_send(events)` and discards the result with `let _ = ...`. When the mpsc channel is full (capacity 256 in `mod.rs:44`), `blocking_send` will block the notify thread. However, the callback runs on notify's internal synchronous thread — indefinitely blocking it stalls all subsequent file events from being delivered to the daemon.

If blocking is undesirable, a `try_send` approach would silently drop events. The current code actually blocks, which is also problematic but for different reasons (notify thread stall). The issue is that there is no logging or back-pressure signal when the channel is full.

**Fix:** Use `try_send` and log a warning when events are dropped due to back-pressure:

```rust
move |result: DebounceEventResult| {
    if let Ok(events) = result {
        if tx.try_send(events).is_err() {
            // Channel full or closed; event batch dropped.
            // This can happen if the processor is slow relative to event rate.
            eprintln!("[watcher] event channel full, dropping batch");
        }
    }
}
```

Alternatively, keep `blocking_send` but add a timeout or log when it blocks. The key requirement is that the notify thread must not stall silently.

---

### WR-03: `remove_file` Increments `embedding_errors_total` on Delete Failure

**File:** `src/daemon/processor.rs:274-276`

**Issue:** When `delete_chunks_for_file` fails in `remove_file`, the code increments `embedding_errors_total`. A delete failure is not an embedding error — it is a store/database error. This mislabeling makes it impossible to distinguish embedding API failures from database write failures in alerting. Operators will investigate the embedding pipeline when the actual issue is a database problem.

**Fix:** Introduce a dedicated metric or at minimum use a log-only approach for store errors vs. embedding errors. If adding a new metric is out of scope, remove the `increment_embedding_errors` call from `remove_file` and keep only the `warn!` log:

```rust
if let Err(e) = store.delete_chunks_for_file(&relative_str).await {
    tracing::warn!(file = %relative_str, error = %e, "failed to remove chunks for deleted file");
    // Do not increment embedding_errors_total -- this is a store error, not an embedding error
}
```

---

### WR-04: Daemon Does Not Validate `--bind` Address Before Spawning Tasks

**File:** `src/daemon/mod.rs:60-66`

**Issue:** The HTTP server bind failure is handled inside the spawned task — it logs an error and returns, but does not cancel the `CancellationToken`. This means that if `--bind 0.0.0.0:9090` fails (e.g., port already in use), the daemon silently continues running with no HTTP server and no `/health` or `/metrics` endpoints. The event processor task keeps running, consuming API quota and disk, with no observable failure.

```rust
Err(e) => {
    tracing::error!(error = %e, bind = %bind_clone, "failed to bind HTTP server");
    return; // task exits, but daemon continues without HTTP
}
```

**Fix:** Cancel the token on bind failure so the daemon shuts down cleanly:

```rust
Err(e) => {
    tracing::error!(error = %e, bind = %bind_clone, "failed to bind HTTP server");
    http_token.cancel(); // signal shutdown to all tasks
    return;
}
```

---

## Info

### IN-01: `setup_shutdown` Only Handles SIGINT (Ctrl-C), Not SIGTERM

**File:** `src/daemon/shutdown.rs:10`

**Issue:** `tokio::signal::ctrl_c()` only listens for SIGINT (Ctrl-C / keyboard interrupt). SIGTERM — the standard signal sent by `systemctl stop`, `launchctl stop`, Docker, and process supervisors — is not handled. The daemon will not shut down gracefully when managed by a supervisor.

**Fix:** Add SIGTERM handling via `tokio::signal::unix`:

```rust
use tokio::signal::unix::{signal, SignalKind};

async fn wait_for_signal() {
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = sigterm.recv() => {}
    }
}
```

Then call `wait_for_signal().await` in place of `tokio::signal::ctrl_c().await`.

---

### IN-02: `count_distinct_files` Loads All File Paths into Memory to Count Them

**File:** `src/pipeline/store.rs:304-307`

**Issue:** `count_distinct_files` calls `get_all_file_paths`, which fetches every `file_path` value from the entire table into a `HashSet` in memory, solely to count the distinct entries. For a large vault this allocates O(n) memory unnecessarily for a count operation.

**Fix:** If LanceDB exposes a `COUNT(DISTINCT ...)` SQL aggregate, use it. Otherwise, this is acceptable for v1 but worth a TODO comment noting the O(n) nature for future optimization.

---

### IN-03: `main.rs` Uses `std::env::current_dir().unwrap()` Without Error Handling

**File:** `src/main.rs:382`, `src/main.rs:435`

**Issue:** `std::env::current_dir().unwrap()` panics if the current directory has been deleted or is otherwise inaccessible. This is called in the `Search` and `Status` command paths as a fallback for `--data-dir`. While uncommon, it produces an uninformative panic rather than a user-friendly error message.

**Fix:** Replace `unwrap()` with proper error propagation:

```rust
let data_dir = cli.data_dir.clone()
    .unwrap_or_else(|| std::env::current_dir()
        .expect("current directory is not accessible")
        .join(".local-index"));
```

Or propagate as an `anyhow` error:

```rust
let cwd = std::env::current_dir()
    .map_err(|e| anyhow::anyhow!("Cannot determine current directory: {}", e))?;
let data_dir = cli.data_dir.clone().unwrap_or_else(|| cwd.join(".local-index"));
```

---

_Reviewed: 2026-04-10_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
