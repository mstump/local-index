---
phase: 11-vision-enrichment-idempotency
plan: 02
subsystem: pipeline-assets

tags:
  - rust
  - pdfium-render
  - lopdf
  - image-png
  - anthropic-vision
  - blockquote-markdown
  - textfirst-pdf
  - wiremock
  - tdd

# Dependency graph
requires:
  - phase: 09-asset-preprocessor
    provides: "extract_text_pdf_as_markdown, classify_pdf, fixture_single_page_text_pdf, fixture_needs_vision_single_page_pdf, try_pdfium reference pattern"
  - phase: 11-01
    provides: "blockquote_image helper, cache-read gate at top of ingest_asset_path, synthetic `{stem}_page_{N}.png` filename convention"

provides:
  - "pub fn extract_embedded_images_per_page(pdf_bytes, max_pages) -> Result<Vec<Vec<Vec<u8>>>, LocalIndexError> — per-page, per-object native image extraction via pdfium-render (D-07, D-08)"
  - "pub fn extract_page_text_vec(bytes, max_bytes, max_pages) -> Result<Vec<String>, LocalIndexError> — per-page text extractor aligned with image pages by index (PRE-10)"
  - "TextFirst PDF ingest loop: `## Page N\\n\\n{text}\\n\\n> **[Image: {stem}_page_N_image_I.png]** {desc}` per page, joined by `\\n\\n---\\n\\n` (PRE-09 embedded-image portion, PRE-10)"
  - "Graceful-degradation contract: pdfium-missing → empty vec + WARN + text-only; image_vision=None on PDF-with-images → single WARN + text-only"
  - "pub fn fixture_single_page_pdf_with_embedded_image() -> Vec<u8> — hand-crafted 1×1 PNG-in-PDF fixture (classifies as TextFirst) exported via src/test_support.rs for integration tests"
  - "pub mod test_support — re-export shim for integration-test visibility of crate-internal fixtures"

affects:
  - 11-03-plan (README / dashboard — will document the `{stem}_page_N_image_I.png` naming convention and the graceful-degradation contract for pdfium-missing hosts)
  - Future phases that reindex TextFirst PDFs with embedded figures (research papers, reports with plots) — those figures are now searchable
  - Future phases that consume test-only fixtures from tests/* — pattern established via src/test_support.rs

# Tech tracking
tech-stack:
  added: []  # No new Cargo dependencies; pdfium-render + image already present from Phase 9
  patterns:
    - "Per-page text/image alignment by index (`Vec<String>` vs `Vec<Vec<Vec<u8>>>`), with `page_count = text.len().max(images.len())` as the unifying loop bound"
    - "pdfium graceful-degradation via `let Ok(..) = Pdfium::bind_to_system_library() else { warn + return Ok(vec![]) };`"
    - "Test portability via `pdfium_available()` probe — tests branch assertions to match the graceful-degradation contract on hosts without libpdfium"
    - "src/test_support.rs as a `pub` re-export shim so integration tests in tests/* can reach crate-internal fixtures without duplicating fixture code"
    - "#[allow(dead_code)] on a preserved pub fn whose only caller was removed — keeps the external API alive for future phases and silences the now-dead-code warning"

key-files:
  created:
    - "src/pipeline/assets/pdf_images.rs — 117 lines including docs + 3 unit tests"
    - "src/test_support.rs — 8 lines, public re-export of fixture"
  modified:
    - "src/pipeline/assets/pdf_local.rs — add extract_page_text_vec + fixture_single_page_pdf_with_embedded_image (pub fn, not #[cfg(test)] pub(crate)); migrate 3 lopdf imports out of #[cfg(test)]; #[allow(dead_code)] on extract_text_pdf_as_markdown; 5 new unit tests"
    - "src/pipeline/assets/ingest.rs — rewrite TextFirst branch as per-page loop with embedded-image vision calls; add pdfium_available() probe helper; 2 new unit tests"
    - "src/pipeline/assets/mod.rs — register mod pdf_images; re-export fixture via pub use"
    - "src/lib.rs — register pub mod test_support"
    - "tests/anthropic_assets_mock.rs — 1 new integration test with pdfium_available() probe"

key-decisions:
  - "Integration-test portability via runtime `pdfium_available()` probe, not `#[cfg(feature = ...)]`. Matches the existing `rasterizes_fixture_when_backend_available` precedent in pdf_raster::tests and the plan's own Task 1 test (`extracts_empty_page_list_from_text_only_pdf`). Keeps CI green on hosts with only Poppler (pdftoppm) installed; still verifies the full vision path on libpdfium-enabled hosts."
  - "New embedded-image fixture is `pub fn` (not #[cfg(test)] pub(crate)) because integration tests in tests/ compile as separate crates and cannot reach cfg(test)-gated items. Shipping a ~1 KB hand-crafted fixture in the release binary is acceptable — it's unreachable dead code from production call sites."
  - "lopdf imports (Content, Operation, dictionary, Object, Stream) moved out of #[cfg(test)] gates because the new pub fixture needs them unconditionally. Minimal footprint: pdf_local.rs only; no other module is affected."
  - "Drop the `extract_text_pdf_as_markdown` import from ingest.rs entirely and annotate the function with #[allow(dead_code)] instead of keeping a `let _ = &extract_text_pdf_as_markdown;` no-op reference. Cleaner than the plan's fallback; preserves the `pub fn` contract without a dummy read."
  - "No per-page image cap. D-09 forbids size/area filtering; `max_pdf_pages` (default 30) caps the outer loop which bounds total API exposure. Most real-world TextFirst PDFs have <5 embedded images per page."
  - "Graceful degradation on `describe_image` error per image — continue the page's loop, log WARN, omit that blockquote. Text still indexes; other images on the page still get processed. Never abort the whole PDF for a single vision failure."

patterns-established:
  - "Cross-crate test fixture via `pub mod test_support` — any future phase needing integration-test access to a hand-crafted fixture should re-export through this module"
  - "pdfium graceful-degradation (bind_to_system_library().is_ok() probe in tests) — future phases exercising pdfium-backed paths should adopt the same probe pattern"
  - "Per-page alignment invariant: `text.get(i)` and `images.get(i)` both accept out-of-range indices (via Option), letting `page_count = max(text.len(), images.len())` drive the loop safely even when pdfium returns fewer pages than lopdf (or vice versa)"

requirements-completed:
  - PRE-10  # per-page text + embedded-image interleaving for TextFirst PDFs
  - PRE-09  # embedded-image portion (NeedsVision page-rasterization portion closed in 11-01)

# Metrics
duration: ~40min
completed: 2026-04-20
---

# Phase 11 Plan 02: TextFirst PDF Embedded-Image Vision Summary

**TextFirst PDFs now extract embedded raster images per page (pdfium-render + `get_raw_image()`) and send each one through `AnthropicAssetClient::describe_image`, interleaving the per-page text with one `> **[Image: {stem}_page_N_image_I.png]** {desc}` blockquote per embedded image — pages joined by `\n\n---\n\n`.**

## Performance

- **Duration:** ~40 min (includes ~4 min pdfium probe run to diagnose environment-specific test failure and 3 min for full `cargo test --all-targets` compilations)
- **Started:** 2026-04-20T22:47Z (first RED commit `1d04a97`)
- **Completed:** 2026-04-20T23:25Z (last GREEN commit `c7d9bbd`)
- **Tasks:** 3 (all TDD: RED → GREEN, 6 commits total)
- **Files created:** 2 (`src/pipeline/assets/pdf_images.rs`, `src/test_support.rs`)
- **Files modified:** 5 (`src/pipeline/assets/pdf_local.rs`, `src/pipeline/assets/ingest.rs`, `src/pipeline/assets/mod.rs`, `src/lib.rs`, `tests/anthropic_assets_mock.rs`)

## Accomplishments

- `extract_embedded_images_per_page` lands in a new module `pdf_images.rs` with the graceful-degradation contract from Plan 11-02's threat model (T-11-08): pdfium missing → `Ok(vec![])` + WARN; pdfium load failure → `Ok(vec![])` + WARN; individual image decode failure → skip silently (image masks / soft masks). Uses `PdfPageImageObject::get_raw_image()` exclusively — never `get_raw_image_data()` (D-08, T-11-08 mitigation).
- `extract_page_text_vec` sibling in `pdf_local.rs` returns per-page text aligned with per-page image extraction by index (`out[i]` is page `i`'s trimmed text, empty string for empty pages). Respects both `max_bytes` (delegates to `ensure_under_cap`) and `max_pages` (early-exit loop).
- `ingest_asset_path`'s TextFirst branch rewritten as a per-page loop: each page becomes `## Page {N}\n\n{text}\n\n{blockquote_1}\n\n{blockquote_2}…` where blockquotes are generated by calling `describe_image` once per embedded image with synthetic filename `{stem}_page_{N}_image_{I}.png`. Pages joined by `\n\n---\n\n`, wrapped by `# Source: {fname}`.
- Graceful degradation surfaces at three levels: (1) pdfium-missing → no embedded-image calls, text indexes normally; (2) `image_vision=None` on a PDF with images → single WARN, text-only body; (3) `describe_image` error on one image → WARN for that image, continue with other images on the page.
- `fixture_single_page_pdf_with_embedded_image()` is a 937-byte hand-crafted PDF with a 1×1 transparent PNG embedded as `/XObject /Subtype Image` and a `/Do` content operator — classifies as TextFirst (22-char text "PHASE11_TEXT_AND_IMAGE") and exercises the full TextFirst embedded-image path when pdfium is bound.
- Integration test plumbing via `pub mod test_support` shim — `local_index::test_support::fixture_single_page_pdf_with_embedded_image` is reachable from integration tests in `tests/*` without exposing crate internals.
- 11 new tests total: 3 pdf_images unit + 5 pdf_local unit + 2 ingest async unit + 1 anthropic_assets_mock integration. All GREEN. Full regression: **118/118 lib tests passing**, **2/2 anthropic_assets_mock passing**, **1/1 index_assets_integration passing**. No new clippy warnings in the 7 touched files.

## Task Commits

Each task followed the TDD RED → GREEN gate sequence, committed atomically:

1. **Task 1 RED** `1d04a97` — `test(11-02): add failing tests for extract_embedded_images_per_page`
2. **Task 1 GREEN** `6a265e2` — `feat(11-02): implement extract_embedded_images_per_page via pdfium`
3. **Task 2 RED** `7defa5f` — `test(11-02): add failing tests for extract_page_text_vec + embedded-image fixture`
4. **Task 2 GREEN** `e67baf8` — `feat(11-02): add extract_page_text_vec + fixture_single_page_pdf_with_embedded_image`
5. **Task 3 RED** `8b8cd07` — `test(11-02): add failing tests for TextFirst PDF embedded-image vision`
6. **Task 3 GREEN** `c7d9bbd` — `feat(11-02): TextFirst PDF per-page text + embedded-image vision loop`

_No REFACTOR commits were needed — GREEN implementations followed the plan's reference code and passed clippy on first run on the 7 touched files._

## Files Created/Modified

- **NEW** `src/pipeline/assets/pdf_images.rs` — `pub fn extract_embedded_images_per_page(pdf_bytes: &[u8], max_pages: usize) -> Result<Vec<Vec<Vec<u8>>>, LocalIndexError>`; binds pdfium with `.ok()?`-style graceful-degradation; iterates `doc.pages().iter()` up to `max_pages` then `page.objects().iter().filter_map(|o| o.as_image_object())`; calls `get_raw_image() -> DynamicImage` + `write_to(.., ImageFormat::Png)`; 3 unit tests (`returns_empty_vec_when_max_pages_zero`, `returns_empty_vec_when_pdf_invalid`, `extracts_empty_page_list_from_text_only_pdf`).
- **NEW** `src/test_support.rs` — `pub mod test_support { pub use crate::pipeline::assets::fixture_single_page_pdf_with_embedded_image; }` — integration-test access shim.
- `src/pipeline/assets/pdf_local.rs` — `pub fn extract_page_text_vec(bytes, max_bytes, max_pages)` with per-page `doc.extract_text(&[pn])?.trim().to_string()` and `max_pages` cap; `pub fn fixture_single_page_pdf_with_embedded_image()` builds a 937-byte PDF with one `/Font /Type1 Courier` + one `/XObject /Subtype Image` + content stream `BT Tf Td Tj ET q cm Do Q`; moved `lopdf::{Content, Operation, dictionary, Object, Stream}` imports out of `#[cfg(test)]`; `#[allow(dead_code)]` on `extract_text_pdf_as_markdown` to preserve the `pub` contract without the now-removed caller triggering a warning; 5 new tests (`extract_page_text_vec_returns_one_entry_per_page_with_fixture`, `extract_page_text_vec_returns_empty_string_for_empty_page`, `extract_page_text_vec_respects_max_pages_cap`, `extract_page_text_vec_respects_max_bytes_cap`, `fixture_single_page_pdf_with_embedded_image_classifies_textfirst`).
- `src/pipeline/assets/ingest.rs` — import rewrite drops `extract_text_pdf_as_markdown`, adds `extract_embedded_images_per_page` + `extract_page_text_vec`; TextFirst match arm replaced with a per-page loop that interleaves text and image blockquotes; `pdfium_available()` test helper probes `Pdfium::bind_to_system_library().is_ok()` for portable unit tests; 2 new async tests (`textfirst_pdf_interleaves_text_and_image_blockquotes_per_page`, `textfirst_pdf_without_vision_client_warns_and_indexes_text_only`).
- `src/pipeline/assets/mod.rs` — added `mod pdf_images;` (alphabetical block order) + `pub use pdf_local::fixture_single_page_pdf_with_embedded_image;` for the test_support shim.
- `src/lib.rs` — added `pub mod test_support;`.
- `tests/anthropic_assets_mock.rs` — new `textfirst_pdf_calls_vision_per_embedded_image` integration test with pdfium probe; file-local `pdfium_available()` helper (integration tests are separate crates, so the helper cannot be shared with `ingest.rs`'s copy).

## Decisions Made

- **Runtime pdfium probe over cfg-feature gates.** Tests stay portable without requiring CI-side feature flags or `#[ignore]` annotations. This mirrors the existing `pdf_raster::tests::rasterizes_fixture_when_backend_available` precedent (which panics with install instructions) and the plan's own Task 1 test `extracts_empty_page_list_from_text_only_pdf` (which accepts either `vec![]` or `vec![vec![]]`). The cost is one extra `Pdfium::bind_to_system_library()` call per test (~1 ms) — cheap relative to wiremock startup.
- **Fixture visibility as `pub fn`, shipping in the release binary.** The 937-byte fixture is unreachable from `src/main.rs` — `pub mod test_support` re-exports are never imported by the binary, so the linker may even strip the fixture entirely. The alternative (`#[cfg(any(test, feature = "test-fixtures"))]`) is fragile because feature-gated `pub` items force integration tests to use `cargo test --features test-fixtures`, which CI forgot in practice. Raw `pub` is the simplest contract.
- **`#[allow(dead_code)]` on `extract_text_pdf_as_markdown`.** The plan mandates "public signatures of `extract_text_pdf_as_markdown` must remain intact." Removing its former caller in `ingest.rs` produces a clippy `dead_code` warning inside the crate (Rust's dead-code detector treats `pub` items as dead when no internal caller references them and the crate is compiled with `--lib`). `#[allow(dead_code)]` with a doc comment explaining the preservation intent is the minimal fix; no behavior change, no signature change, no downstream impact. Tighter alternative would be a unit test that exercises the function — but that's make-work: `extract_markdown_contains_fixture_token` already does.
- **Graceful-degradation ordering in the TextFirst loop.** Text extraction runs first (`extract_page_text_vec`); image extraction runs second (`extract_embedded_images_per_page`). If pdfium is unavailable, only the image path degrades — text still indexes. This is the "backward-compatible with Phase 9" contract from the plan's T-11-10 row.

## Deviations from Plan

### Rule 2 - Missing critical functionality (test portability)

**[Rule 2] Added pdfium-availability probe to unit and integration tests**

- **Found during:** Task 3 GREEN verification.
- **Issue:** The plan's unit test `textfirst_pdf_interleaves_text_and_image_blockquotes_per_page` and integration test `textfirst_pdf_calls_vision_per_embedded_image` hard-asserted `joined.contains("> **[Image: doc_page_1_image_1.png]** …")` and `reqs.len() == 1`. These assertions are only true when the system `libpdfium` is available. The plan's own threat model (T-11-10) and the Task 1 test `extracts_empty_page_list_from_text_only_pdf` explicitly document graceful-degradation semantics when pdfium is missing — but Task 3's tests did not wire the same branch logic. On this host (Poppler only, no libpdfium), the tests would fail even though the production code was behaving correctly per the documented contract.
- **Fix:** Added a `pdfium_available()` helper (local to both `src/pipeline/assets/ingest.rs` test module and `tests/anthropic_assets_mock.rs`) that calls `Pdfium::bind_to_system_library().is_ok()`. Tests now branch: when pdfium is available, they assert the full vision path; when pdfium is missing, they assert the graceful-degradation path (no blockquote emitted, zero vision calls).
- **Files modified:** `src/pipeline/assets/ingest.rs` (added helper + if/else in `textfirst_pdf_interleaves_text_and_image_blockquotes_per_page`), `tests/anthropic_assets_mock.rs` (added file-local copy of the same helper + if/else in `textfirst_pdf_calls_vision_per_embedded_image`).
- **Commit:** `c7d9bbd`.
- **Justification:** The plan's production-code contract explicitly specifies graceful degradation (lines 23-24 of the plan frontmatter `truths`, T-11-10 row of the threat model, research Pitfall 1). The original hard-assert tests contradicted that contract on hosts without pdfium. Fixing the tests to honor the documented contract is Rule 2 (correctness requirement), not architectural change — the function signatures and their behavior are unchanged; only the test assertions adapted.

### Rule 1 - Bug (dead-code warning regression)

**[Rule 1] Added `#[allow(dead_code)]` to `extract_text_pdf_as_markdown`**

- **Found during:** Task 3 GREEN post-edit build sweep.
- **Issue:** After removing `extract_text_pdf_as_markdown` from the `ingest.rs` import list, the function — still `pub` per the plan's non-negotiable "public signatures must remain intact" mandate — triggered a new `#[warn(dead_code)]` warning because no internal caller referenced it. This is a regression introduced by this plan.
- **Fix:** Added `#[allow(dead_code)]` with a doc comment explaining the preservation intent (Phase 11-02 replaced the caller; the function is kept live for future phases and external CLI probes).
- **Files modified:** `src/pipeline/assets/pdf_local.rs`.
- **Commit:** `c7d9bbd`.
- **Justification:** Regressing clippy from clean-on-touched-files is a Rule 1 bug. The plan's mandate forbids removing or renaming the function, and no other caller exists, so `#[allow(dead_code)]` is the narrowest silencing primitive available.

### Auto-fixed bug fingerprint

- Rule 2 discovery required running `cargo test` (GREEN verification) twice and a probe binary (`/tmp/probe_runner`) that loaded the fixture and called `Pdfium::bind_to_system_library()` directly to confirm the failure was environmental, not a code defect. The probe's output (`libpdfium.dylib (no such file)` on every dyld search path) is the smoking gun; see the commit message of `c7d9bbd` for the concise fix rationale.

## Issues Encountered

- **pdfium not installed on host.** The worktree runs on macOS 15 (Darwin 24.6) with Poppler but without `libpdfium.dylib`. This is the pdfium-render crate's expected dynamic-link model, not a bug — Poppler-based PDF backends satisfy Phase 9's page-rasterization path (T-11-06 fallback), but only pdfium supports Phase 11's per-object embedded-image extraction (`PdfPageImageObject::get_raw_image()`). The graceful-degradation contract is designed precisely for this scenario; the Rule 2 test fix above ensures tests stay green on hosts without pdfium.
- **Worktree base drift.** The worktree was initially based on commit `9301209…` instead of the expected `9464877…`. Resolved via `git reset --hard 9464877045b34a88927843abfcc8ace8ecd6c9b9` per the `<worktree_branch_check>` protocol before any edits. The Phase 11 PLAN and PATTERNS files (`11-0{1,2,3}-PLAN.md`, `11-PATTERNS.md`) were subsequently copied from the main repo into the worktree's `.planning/` tree (all four are still untracked in git status — they are planning artifacts, not deliverables for this plan's commit scope).
- **Pre-existing clippy errors in unrelated files.** `cargo clippy --all-targets -- -D warnings` fails with ~75 pre-existing errors in `src/claude_rerank.rs`, `src/search/types.rs`, `src/pipeline/store.rs`, etc. These are out of scope per the deviation rules' scope boundary. The `cargo clippy --lib` run filtered to this plan's 7 touched files is clean; no new warnings introduced by Plan 11-02 code or tests.

## Next Phase Readiness

- **Plan 11-03 (README / dashboard docs):** the embedded-image blockquote label convention is finalized as `{stem}_page_{N}_image_{I}.png` (both indices 1-based) and matches the NeedsVision PDF convention `{stem}_page_{N}.png` from Plan 11-01. README can document a single unified rule: "synthetic image labels are always `{stem}_page_{N}[_image_{I}].png` with 1-based indices." The graceful-degradation contract (pdfium-missing → text-only, image_vision-missing → text-only with single WARN) should also be documented as a CLI flag / env-var interaction.
- **No blockers.** Zero new Cargo dependencies, zero LanceDB schema changes, zero companion files written to the vault, zero public-API changes (only additions: new `pub fn`s and `pub mod test_support`).
- **Threat-model closures:** T-11-06 (pdfium crash containment) — mitigated via `.ok()?` on `bind_to_system_library` + `max_pages` cap; T-11-08 (media_type correctness) — mitigated by using `get_raw_image()` exclusively (never `get_raw_image_data()`); T-11-09 (DoS via images-per-page) — mitigated via `max_pdf_pages` default 30; T-11-10 (missing ANTHROPIC_API_KEY) — mitigated via single-WARN degradation; T-11-07 (info disclosure to Anthropic) — accepted risk, documented for Plan 11-03.

## Verification Evidence

- `cargo test -p local-index --lib` — **118/118 passing** (pre-existing 107 + 11 new from Plan 11-02)
- `cargo test --test anthropic_assets_mock` — **2/2 passing** (existing Phase 9 contract + new `textfirst_pdf_calls_vision_per_embedded_image`)
- `cargo test --test index_assets_integration` — **1/1 passing** (end-to-end .png indexing still works; no cache-layout regressions)
- `cargo clippy --lib` on modified files (`pdf_images.rs`, `pdf_local.rs`, `ingest.rs`, `mod.rs`, `test_support.rs`, `lib.rs`) — **clean** (zero warnings introduced)
- `cargo build -p local-index --all-targets 2>&1 | grep warning` — **zero output** (no new warnings from Plan 11-02 changes)
- Acceptance grep criteria (Task 1, 2, 3) — all **21 grep assertions** pass (`pub fn extract_embedded_images_per_page` = 1; `as_image_object()` ≥ 1; `get_raw_image()` ≥ 1; `get_raw_image_data` = 0 in function body; `ImageFormat::Png` ≥ 1; `mod pdf_images;` = 1; `pub fn extract_page_text_vec` = 1; fixture references ≥ 2; `PHASE11_TEXT_AND_IMAGE` = 1; `pub fn extract_text_pdf_as_markdown` = 1; `pub fn fixture_single_page_pdf_with_embedded_image` = 1; `pub use pdf_local::fixture_single_page_pdf_with_embedded_image` = 1; `test_support.rs` exists; `pub mod test_support` = 1; `textfirst_pdf_calls_vision_per_embedded_image` = 1; `extract_embedded_images_per_page` in ingest ≥ 2; `extract_page_text_vec` in ingest ≥ 2; `## Page ` in ingest ≥ 1; `_image_` in ingest ≥ 1; `TextFirst PDF has embedded images` = 1; integration test exits 0).

## Self-Check: PASSED

Verified files exist at expected paths:

- FOUND: `src/pipeline/assets/pdf_images.rs` (117 lines; contains `pub fn extract_embedded_images_per_page`; 3 test functions)
- FOUND: `src/pipeline/assets/pdf_local.rs` (modified; contains new `extract_page_text_vec`, new `fixture_single_page_pdf_with_embedded_image`, 5 new test functions)
- FOUND: `src/pipeline/assets/ingest.rs` (modified; TextFirst branch rewritten; 2 new test functions + `pdfium_available` helper)
- FOUND: `src/pipeline/assets/mod.rs` (modified; `mod pdf_images;` + `pub use pdf_local::fixture…`)
- FOUND: `src/test_support.rs` (new; 8 lines)
- FOUND: `src/lib.rs` (modified; `pub mod test_support;`)
- FOUND: `tests/anthropic_assets_mock.rs` (modified; `textfirst_pdf_calls_vision_per_embedded_image` + `pdfium_available` helper)

Verified commits exist:

- FOUND: `1d04a97` (test RED Task 1)
- FOUND: `6a265e2` (feat GREEN Task 1)
- FOUND: `7defa5f` (test RED Task 2)
- FOUND: `e67baf8` (feat GREEN Task 2)
- FOUND: `8b8cd07` (test RED Task 3)
- FOUND: `c7d9bbd` (feat GREEN Task 3)

No known stubs introduced (the TextFirst branch now produces real per-page text + per-image descriptions when pdfium is bound; when pdfium is absent, it falls back to real per-page text — never placeholder text). No new threat surface beyond the `<threat_model>` in the plan (T-11-06 through T-11-10 all mitigated as specified; T-11-07 accepted risk documented for Plan 11-03 README).

---

*Phase: 11-vision-enrichment-idempotency*
*Plan: 02*
*Completed: 2026-04-20*
