---
phase: 04-daemon-mode-observability
fixed_at: 2026-04-10T00:00:00Z
review_path: .planning/phases/04-daemon-mode-observability/04-REVIEW.md
iteration: 1
findings_in_scope: 6
fixed: 6
skipped: 0
status: all_fixed
---

# Phase 04: Code Review Fix Report

**Fixed at:** 2026-04-10
**Source review:** .planning/phases/04-daemon-mode-observability/04-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 6 (2 Critical, 4 Warning)
- Fixed: 6
- Skipped: 0

## Fixed Issues

### CR-01: SQL Injection via Unsanitized File Path in LanceDB Filter Strings

**Files modified:** `src/pipeline/store.rs`
**Commit:** 34cb86e
**Applied fix:** Added `escape_sql_string(s: &str) -> String` helper that doubles single quotes (SQL standard escaping). Applied it to both filter-interpolation sites: `get_hashes_for_file` (`.only_if(...)`) and `delete_chunks_for_file` (`.delete(...)`). Filenames with apostrophes (e.g. `it's a note.md`) now produce a safe filter string `file_path = 'it''s a note.md'` instead of breaking the SQL expression.

---

### CR-02: Double-Increment of `file_events_processed_total` Counter for Remove Events

**Files modified:** `src/daemon/processor.rs`
**Commit:** ebc004b
**Applied fix:** Removed the `metrics::increment_file_events()` call from the end of `remove_file`. All three caller sites (`EventKind::Remove`, `RenameMode::Both`, `RenameMode::From`) already call `metrics::increment_file_events()` after `remove_file` returns, making the internal call a double-count. Added a comment explaining that callers are responsible for incrementing, consistent with `reindex_file` design.

---

### WR-01: Blocking File I/O on the Async Executor Thread

**Files modified:** `src/daemon/processor.rs`
**Commit:** 810bb68
**Applied fix:** Replaced `std::fs::read_to_string(path)` with `tokio::fs::read_to_string(path).await` in `reindex_file`. No signature change needed — the function is already `async fn`. This prevents large file reads from blocking the tokio executor thread and starving the HTTP server and event loop.

---

### WR-02: Watcher Channel Back-Pressure: Silent Event Drop on Full Channel

**Files modified:** `src/daemon/watcher.rs`
**Commit:** a119ff9
**Applied fix:** Replaced `tx.blocking_send(events)` with `tx.try_send(events)` in the notify debouncer callback. When the channel is full or closed, a `tracing::warn!` is emitted so operators can detect back-pressure. This prevents the notify internal thread from stalling indefinitely when the event processor is slow.

---

### WR-03: `remove_file` Increments `embedding_errors_total` on Delete Failure

**Files modified:** `src/daemon/processor.rs`
**Commit:** 1b4107a
**Applied fix:** Removed `metrics::increment_embedding_errors()` from the `delete_chunks_for_file` error path in `remove_file`. A database delete failure is a store error, not an embedding error. The `tracing::warn!` log is retained so the failure remains observable. A comment explains the rationale to prevent future regression.

---

### WR-04: Daemon Does Not Validate `--bind` Address Before Spawning Tasks

**Files modified:** `src/daemon/mod.rs`
**Commit:** ab61cb0
**Applied fix:** Added `http_token.cancel()` in the `Err` arm of the `TcpListener::bind` match, before returning from the spawned HTTP task. This causes the `CancellationToken` to fire, which shuts down the event processor and the daemon cleanly rather than silently continuing without any HTTP observability endpoints.

---

## Skipped Issues

None — all in-scope findings were fixed.

---

_Fixed: 2026-04-10_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
