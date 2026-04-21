---
phase: 11-vision-enrichment-idempotency
plan: 01
subsystem: pipeline-assets

tags:
  - rust
  - tokio
  - sha256
  - blockquote-markdown
  - idempotency
  - anthropic-vision
  - tracing
  - wiremock
  - lancedb-ingest

# Dependency graph
requires:
  - phase: 09-asset-preprocessor
    provides: "ingest_asset_path, cache_path_for_hash, ensure_cache_parent, AnthropicAssetClient, rasterize_pdf_pages_to_png, OcrService"
  - phase: 10-ocr-providers
    provides: "OcrService::{Anthropic,Google}, ocr_scanned_pdf_pages"

provides:
  - "read_cache_if_present(path) async helper with corrupt-cache WARN (D-03)"
  - "Private blockquote_image(filename, description) helper producing canonical `> **[Image: {filename}]** {desc}` wrapper with `> ` prefix on every continuation line (D-04, D-05)"
  - "SHA-256 cache-read gate at top of ingest_asset_path that short-circuits every OCR / vision API call on a cache hit (PRE-04)"
  - "Standalone .png/.jpg/.jpeg/.webp assets emit blockquote-wrapped vision description (PRE-11, PRE-12)"
  - "NeedsVision PDF pages emit per-page blockquote with synthetic {stem}_page_{N}.png label; `\\n\\n---\\n\\n` separator preserved (D-04, D-06; PRE-09 NeedsVision portion)"

affects:
  - 11-02-plan (TextFirst PDF embedded-image vision — will reuse blockquote_image and the cache-read gate is already in place)
  - 11-03-plan (dashboard / README documentation — will reference the new cache-hit log line)
  - any future asset re-ingest path (double-index prevention)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Cache-read gate above per-extension branching (hash once, short-circuit on hit)"
    - "Blockquote-wrapped synthetic markdown for every vision-generated body (single canonical format)"
    - "Structured `tracing::warn!(corrupt_cache = true, path, error)` on any non-NotFound cache IO failure"

key-files:
  created: []
  modified:
    - "src/pipeline/assets/cache.rs — add read_cache_if_present + 5 tokio tests"
    - "src/pipeline/assets/ingest.rs — add blockquote_image helper; restructure ingest_asset_path with top-of-function cache-read gate; wrap standalone image + NeedsVision PDF outputs in blockquote; 7 new tests"

key-decisions:
  - "Cache-read gate runs immediately after size-gate, before fname/ext extraction, so even unsupported extensions never touch the API when their bytes are already cached."
  - "Empty or whitespace-only cache files are treated as corruption (not as valid empty cache); `trim().is_empty()` → WARN with `corrupt_cache = true` → refetch."
  - "Non-NotFound IO errors (e.g. cache path is a directory) log WARN but fall through to refetch rather than abort, preserving availability."
  - "Continuation lines in multi-line vision descriptions are prefixed with `> ` (not just the first line) so the whole block reads as one markdown blockquote, matching CommonMark blockquote semantics."
  - "NeedsVision PDF synthetic filename format is `{stem}_page_{N}.png` (1-based), reusing the stem from `Path::new(fname).file_stem()`; `.unwrap_or(\"doc\")` protects against unusual names without introducing failures."

patterns-established:
  - "Read-before-write cache gate in async pipelines (hash source bytes → derived path → `read_cache_if_present` → short-circuit to existing `chunk_markdown` call)"
  - "Small private formatting helper (`blockquote_image`) above the public entrypoint, mirroring `media_type_for_image` positioning"
  - "WARN-level tracing for corrupt-cache paths with bounded structured fields (never logs file contents)"
  - "Tests use `Sha256` directly + `cache_path_for_hash` + `ensure_cache_parent` to pre-seed cache files deterministically"

requirements-completed:
  - PRE-04
  - PRE-11
  - PRE-12
  - PRE-09  # standalone-image + NeedsVision PDF portions; TextFirst embedded-image portion deferred to 11-02

# Metrics
duration: 12min
completed: 2026-04-20
---

# Phase 11 Plan 01: Vision Idempotency & Blockquote Wrap Summary

**Cache-read gate above per-extension branching in `ingest_asset_path` plus canonical `> **[Image: {filename}]** {desc}` blockquote wrapping for standalone images and NeedsVision PDF OCR pages.**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-04-20T21:16:29Z (first RED commit)
- **Completed:** 2026-04-20T21:27:54Z (last GREEN commit)
- **Tasks:** 2 (both TDD, 4 commits total)
- **Files modified:** 2 (`src/pipeline/assets/cache.rs`, `src/pipeline/assets/ingest.rs`)

## Accomplishments

- `read_cache_if_present` helper lands in `cache.rs` with the D-03 semantics: silent miss on NotFound, WARN + refetch on every other failure mode (empty, whitespace-only, directory path, other IO).
- `ingest_asset_path` now hashes the source bytes once, builds the cache path once, and consults the cache **before** any per-extension branch — so unchanged PDF/PNG/JPG bytes incur zero OCR and zero vision API calls on subsequent runs (PRE-04 closed).
- Standalone `.png` / `.jpg` / `.jpeg` / `.webp` assets emit synthetic markdown of the form `# {fname}\n\n> **[Image: {fname}]** {vision-desc}\n`, with multi-line descriptions preserving the `> ` prefix on every continuation line (PRE-11 + PRE-12 closed).
- NeedsVision PDF OCR pages each become `> **[Image: {stem}_page_{N}.png]** {ocr-text}` with the unchanged `\n\n---\n\n` page separator between pages (D-04, D-06; PRE-09 NeedsVision portion closed — TextFirst embedded-image portion moves to Plan 11-02).
- 12 new unit tests added (5 in `cache.rs`, 7 in `ingest.rs`); 2 pre-existing ingest tests remain green; full library suite 108/108 passing.

## Task Commits

Each task followed the TDD RED → GREEN gate sequence, committed atomically:

1. **Task 1 RED: failing tests for `read_cache_if_present`** — `77170c2` (test)
2. **Task 1 GREEN: implement `read_cache_if_present`** — `adc810d` (feat)
3. **Task 2 RED: failing tests for blockquote wrap + cache-read gate** — `4b7f3c2` (test)
4. **Task 2 GREEN: cache-read gate + blockquote wrapping in `ingest_asset_path`** — `62d5587` (feat)

_No REFACTOR commits were needed — GREEN implementations matched the plan's reference code verbatim and passed clippy without cleanup._

## Files Created/Modified

- `src/pipeline/assets/cache.rs` — added `pub async fn read_cache_if_present(&Path) -> Option<String>` with `tracing::warn!(corrupt_cache = true, ...)` on empty / whitespace / non-NotFound IO; 5 new `#[tokio::test]` cases covering hit, empty, whitespace, NotFound (silent), and directory paths.
- `src/pipeline/assets/ingest.rs` — added private `blockquote_image` formatter; extended imports with `read_cache_if_present`; moved SHA-256 computation from tail-of-function (cache write) to top-of-function (cache read) so every downstream API branch is gated by the cache; wrapped standalone image output in `format!("# {fname}\n\n{block}\n", ...)`; rewrote the NeedsVision PDF branch to map each OCR page string through `blockquote_image(&format!("{stem}_page_{}.png", i + 1), &text)` before the `---` join; added 7 new tests (3 pure-helper + 4 async integration with `wiremock` + `tempfile`).

## Decisions Made

- Treat empty / whitespace-only cache files as corruption rather than "valid zero-byte cache": the caller has already paid the API cost to produce a synthetic body, so an empty file is a disk write that was interrupted or a manual tamper.
- Use `Path::new(fname).file_stem().and_then(|s| s.to_str()).unwrap_or("doc")` rather than propagating a parse error, because the outer function already guarantees `fname` is a valid filename — the fallback only activates on pathological inputs and keeps the ingest path total.
- Use `!s.trim().is_empty()` rather than `!s.is_empty()` for cache-hit detection: a PDF rasterizer or mocked response that produced `"   \n"` is functionally a miss, and the cost of one extra refetch is negligible versus emitting chunks whose body is whitespace.
- Preserve the existing cache-write path unchanged (`ensure_cache_parent` + `tokio::fs::write`) but move it after the new gate, so the write only runs on misses. This keeps the D-02 write semantics intact and avoids re-ordering unrelated error handling.

## Deviations from Plan

None — plan executed exactly as written. Each task's `<action>` block translated to a minimal edit, and every acceptance criterion passed on the first green run after the matching GREEN commit.

One note on acceptance criterion tooling: the plan's awk check for "`Sha256::new()` appears before `let fname = asset_rel`" picks the last match on each side, which is misleading because the test module also contains a `Sha256::new()` helper. The **first** occurrences (line 81 for `Sha256::new()` in the function body, line 98 for `let fname = asset_rel`) confirm the SHA-256 computation is correctly positioned above fname extraction. Fix was to manually verify first-match ordering (`awk '{ if(a==0) a=NR ... }'`), not a code change — documented here so Plan 11-02 can update the criterion if it reuses this pattern.

## Issues Encountered

- **Clippy dead-code warning after Task 1 GREEN:** `read_cache_if_present` was introduced in `cache.rs` but not yet wired into `ingest.rs` until Task 2. The `dead_code` lint warned between commits — expected and resolved by Task 2's import. No action needed.
- **Pre-existing clippy errors in unrelated files** (`src/claude_rerank.rs`, `src/search/types.rs`, etc.) flag under `-D warnings` but are out of scope per the deviation rules' scope boundary. The `cargo clippy --lib` run on this plan's two modified files is clean; no new warnings introduced.
- Worktree was initially based on an older commit than the phase head (`32b320a` vs. `980baea`); resolved via `git reset --hard 980baea4b49cfa98766f6b0d4b82e8205043b281` per the `<worktree_branch_check>` protocol before any edits. Plan files (`11-0{1,2,3}-PLAN.md`, `11-PATTERNS.md`) were copied from the main repo into the worktree's `.planning/` tree after the reset.

## Next Phase Readiness

- **Plan 11-02 (TextFirst PDF embedded-image vision):** the `blockquote_image` helper is in place and ready to wrap each embedded-image vision call; the cache-read gate already protects the entire PDF branch (TextFirst or NeedsVision) from redundant vision calls, so Plan 11-02's per-page embedded-image scan can run under the same gate without needing its own cache surface.
- **Plan 11-03 (docs / README):** the `asset cache hit; skipping API` debug log line is now emitted and can be referenced in documentation. The exact cache layout paragraph in README should also mention that empty / corrupt cache files trigger a refetch (D-03 behavior, now observable).
- No blockers. No schema changes. No new dependencies. No public-API changes.

## Verification Evidence

- `cargo test -p local-index --lib pipeline::assets::` — **24/24 passing** (pre-existing + 12 new)
- `cargo test -p local-index --lib` — **108/108 passing** (no regressions)
- `cargo test --test anthropic_assets_mock` — **1/1 passing** (Phase 9 wiremock contract intact)
- `cargo test --test index_assets_integration` — **1/1 passing** (end-to-end .png indexing still works)
- `cargo clippy --lib` on modified files (`src/pipeline/assets/cache.rs`, `src/pipeline/assets/ingest.rs`) — **clean** (no new warnings introduced)

## Self-Check: PASSED

Verified files exist at expected paths:

- FOUND: `src/pipeline/assets/cache.rs` (contains `read_cache_if_present`, 5 test functions matching `read_cache_if_present_*`)
- FOUND: `src/pipeline/assets/ingest.rs` (contains `blockquote_image`, 7 new test functions)

Verified commits exist:

- FOUND: `77170c2` (test RED Task 1)
- FOUND: `adc810d` (feat GREEN Task 1)
- FOUND: `4b7f3c2` (test RED Task 2)
- FOUND: `62d5587` (feat GREEN Task 2)

No known stubs introduced. No new threat surface beyond the `<threat_model>` table in the plan (T-11-01 through T-11-05 all mitigated as specified).

---

*Phase: 11-vision-enrichment-idempotency*
*Plan: 01*
*Completed: 2026-04-20*
