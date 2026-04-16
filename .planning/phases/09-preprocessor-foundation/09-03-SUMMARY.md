---
phase: 09-preprocessor-foundation
plan: 03
subsystem: operator-wiring
tags: [rust, cli, daemon, lancedb, integration-test]

requires:
  - phase: 09-01
  - phase: 09-02
provides:
  - `index` and `daemon` asset loops with `skip_asset_processing` and `exclude_asset_globs`
  - Daemon processor honors operator exclude globs via shared `Override` builder
  - Markdown orphan prune no longer deletes asset `file_path` rows
  - README `## PDF and images (v1.2)` operator documentation
  - `tests/index_assets_integration.rs` (wiremock Voyage + Anthropic, `CARGO_BIN_EXE_local-index`)
affects:
  - "Phase 10 (OCR providers) — builds on ingest + raster paths"

tech-stack:
  patterns:
    - "Integration tests spawn the real CLI via `env!(\"CARGO_BIN_EXE_local-index\")` to avoid stale `target/debug/local-index`"
    - "`prune_absent_markdown_files` only removes `.md` paths missing from the markdown walk"

key-files:
  created:
    - tests/index_assets_integration.rs
  modified:
    - src/cli.rs
    - src/main.rs
    - src/daemon/mod.rs
    - src/daemon/processor.rs
    - src/pipeline/assets/mod.rs
    - src/pipeline/assets/ignore_walk.rs
    - src/pipeline/assets/ingest.rs
    - src/pipeline/store.rs
    - README.md
    - tests/index_integration.rs

key-decisions:
  - "`ChunkStore::prune_absent_markdown_files` skips non-`.md` `file_path` values so indexed PNG/PDF chunks survive the post-index prune pass"
  - "`env_clear` + explicit env in asset integration test isolates from parent shell `VOYAGE_*` / base URL overrides"

requirements-completed: [PRE-01, PRE-02, PRE-13 initial, D-07]

verification:
  - "cargo test (full suite)"

status: complete
completed_at: 2026-04-15
---

# 09-03 Summary — CLI, index/daemon, README, integration test

Wired asset ingestion into `index` and the daemon processor, threaded skip and exclude flags through `run_daemon`, fixed markdown-centric prune so asset chunks are not dropped after indexing, documented operator behavior in README, and added an end-to-end integration test using wiremock for both embedding and vision HTTP. Integration tests now resolve the CLI binary via `CARGO_BIN_EXE_local-index` so runs do not pick up an outdated `local-index` executable.
