---
phase: 04-daemon-mode-observability
plan: 01
subsystem: daemon/metrics
tags: [metrics, prometheus, axum, observability, http]
dependency_graph:
  requires: []
  provides: [metrics-recorder, metrics-constants, http-router, health-endpoint]
  affects: [daemon-watcher, daemon-server, search-instrumentation]
tech_stack:
  added: [metrics 0.24, metrics-exporter-prometheus 0.16, axum 0.8, tower 0.5, tower-http 0.6, notify 8.0, notify-debouncer-full 0.5, tokio-util 0.7, chrono 0.4]
  patterns: [metrics facade with PrometheusHandle, axum Router for /metrics + /health, OnceLock for global recorder in tests]
key_files:
  created:
    - src/daemon/mod.rs
    - src/daemon/metrics.rs
    - src/daemon/http.rs
  modified:
    - Cargo.toml
    - src/lib.rs
decisions:
  - metrics-exporter-prometheus 0.16.2 resolved (plan specified 0.18, Cargo resolved 0.16 due to semver); API is compatible
  - notify 8.0 resolved to 8.2.0; notify-debouncer-full 0.5.0 resolved (plan specified 0.7, Cargo resolved 0.5)
  - Used OnceLock instead of static mut for test handle (Rust 2024 edition disallows static mut refs)
  - HTTP handler uses Arc<PrometheusHandle> for shared ownership in route closure
metrics:
  duration: 20min
  completed: 2026-04-10
  tasks_completed: 2
  tasks_total: 2
  files_changed: 5
  tests_added: 5
  tests_total: 66
requirements:
  - OBS-01
  - OBS-02
  - OBS-03
  - OBS-04
---

# Phase 04 Plan 01: Metrics Foundation and HTTP Router Summary

Prometheus metrics recorder with 12 named metric constants (4 counters, 4 gauges, 4 histograms), custom HDR histogram buckets for all latency-sensitive operations, and axum HTTP router serving /metrics (Prometheus exposition format) and /health endpoints.

## What Was Built

### Task 1: Phase 4 Dependencies and Daemon Module Structure

Added all Phase 4 crate dependencies to Cargo.toml: notify/notify-debouncer-full for file watching, axum/tower/tower-http for HTTP serving, metrics/metrics-exporter-prometheus for Prometheus metrics, tokio-util for shutdown coordination, and chrono for timestamps. Created `src/daemon/` module with `mod.rs`, `metrics.rs`, and `http.rs`. Verified all dependencies compile with `cargo check`.

**Commit:** 22c6ada

### Task 2: Prometheus Metrics Setup and HTTP Router (TDD)

Implemented using TDD (RED -> GREEN):

**RED phase (1364d6b):** Wrote 5 failing tests:
- `test_setup_metrics_returns_handle` - verifies setup_metrics() returns a working PrometheusHandle
- `test_counter_recording` - verifies counter metrics appear in rendered output
- `test_histogram_recording` - verifies histogram metrics appear in rendered output
- `test_health_returns_ok` - verifies GET /health returns 200 with body "ok"
- `test_metrics_returns_200` - verifies GET /metrics returns 200 with valid prometheus text

**GREEN phase (8e8c515):** Implemented:
- `setup_metrics()` installs global Prometheus recorder with custom histogram buckets for embedding_latency_seconds, search_latency_seconds, http_request_duration_seconds, indexing_throughput_chunks_per_second
- 12 metric name constants defined as `pub const &str` to prevent typos across codebase
- 12 convenience recording functions (record_embedding_latency, increment_chunks_indexed, etc.)
- `metrics_router(handle)` creates axum Router with /metrics and /health routes

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Adjusted dependency versions for Cargo resolution**
- **Found during:** Task 1
- **Issue:** Plan specified metrics-exporter-prometheus 0.18 and notify-debouncer-full 0.7, but Cargo resolved 0.16.2 and 0.5.0 respectively due to semver constraints
- **Fix:** Used versions that Cargo could resolve (0.16 and 0.5); API is compatible
- **Files modified:** Cargo.toml

**2. [Rule 1 - Bug] Fixed static mut reference error in test code**
- **Found during:** Task 2 RED phase
- **Issue:** Rust 2024 edition (used by this project) forbids creating shared references to `static mut` variables
- **Fix:** Replaced `static mut HANDLE` + `Once` pattern with `OnceLock<PrometheusHandle>` for safe global test state
- **Files modified:** src/daemon/metrics.rs

**3. [Rule 3 - Blocking] Fixed http crate import in test code**
- **Found during:** Task 2 RED phase
- **Issue:** `use http::Request` does not resolve; the `http` crate is re-exported through axum
- **Fix:** Changed to `use axum::http::Request`
- **Files modified:** src/daemon/http.rs

**4. [Rule 3 - Blocking] Fixed PrometheusBuilder::build() API for test helper**
- **Found during:** Task 2 RED phase
- **Issue:** In metrics-exporter-prometheus 0.16, `build()` returns `(PrometheusRecorder, ExporterFuture)`, not a handle directly
- **Fix:** Used `build_recorder()` + `.handle()` to get a PrometheusHandle without installing globally
- **Files modified:** src/daemon/http.rs

## Verification

All verification criteria met:
1. `cargo check` passes with all new dependencies
2. `cargo test daemon::metrics --lib` passes (3 tests)
3. `cargo test daemon::http --lib` passes (2 tests)
4. `cargo test --lib` passes (66 tests, 0 failures, no regressions)

## Self-Check: PASSED

All 5 key files verified present. All 3 commits verified in git log.
