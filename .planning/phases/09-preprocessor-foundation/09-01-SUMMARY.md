---
phase: 09-preprocessor-foundation
plan: 01
subsystem: infra
tags: [rust, ignore, lopdf, pdf, assets]

requires:
  - phase: 08
    provides: Search UX baseline; no direct coupling
provides:
  - Gitignore-aware asset path discovery under a vault root
  - PDF text-density classification and local markdown extraction
  - Sharded on-disk cache path layout under `data_dir/asset-cache/`
affects:
  - "09-02 (Anthropic vision extraction)"
  - "09-03 (index/daemon wiring)"

tech-stack:
  added: ["ignore 0.4", "lopdf 0.38"]
  patterns:
    - "Vault-relative paths from canonical root + strip_prefix"
    - "OverrideBuilder `!glob` exclusions for operator exclude globs"

key-files:
  created:
    - src/pipeline/assets/mod.rs
    - src/pipeline/assets/ignore_walk.rs
    - src/pipeline/assets/pdf_local.rs
    - src/pipeline/assets/cache.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - src/pipeline/mod.rs
    - src/error.rs
    - src/pipeline/chunker.rs

key-decisions:
  - "Use `ignore::WalkBuilder` + explicit dot-prefix filter to mirror `walker.rs` hidden semantics"
  - "PDF `TextFirst` when printable chars ≥ 12 × page_count (documented in `pdf_local.rs`)"
  - "Disambiguate `chunker` byte slice comparison after `bstr` entered the graph via `ignore`"

patterns-established:
  - "Asset helpers live under `pipeline::assets` with `pub(crate)` facade re-exports"

requirements-completed: [PRE-03, PRE-05, PRE-06]

duration: 45min
completed: 2026-04-15
---

# Phase 9 Plan 01 Summary

**Established the preprocessor asset pipeline building blocks: gitignore-aware discovery, local PDF text extraction with a documented density heuristic, and sharded cache paths — ready for Anthropic wiring in Plan 02.**

## Performance

- **Duration:** ~45 min
- **Tasks:** 4
- **Files modified:** 9

## Accomplishments

- Added `pipeline::assets` with `discover_asset_paths`, PDF classification/extraction, and cache path helpers covered by unit tests.
- Fixed `chunker` code-fence scan to avoid ambiguous `[u8]: AsRef<_>` after new dependencies pulled in `bstr`.

## Task Commits

1. **Task 1: Dependencies and module skeleton** — `8647e8d`
2. **Task 2: Gitignore-aware asset discovery** — `5ceb865`
3. **Task 3: PDF classification + local text extraction** — `7a2ed39`
4. **Task 4: Ephemeral cache helpers** — `4ac5bab`

## Self-Check: PASSED

- `cargo test` — all tests passed (2026-04-15).

## Deviations

- None material; `chunker.rs` change is a compile-fix triggered by the new dependency graph, not a product behavior change.
