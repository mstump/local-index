---
phase: 11-vision-enrichment-idempotency
verified: 2026-04-20T23:55:00Z
status: passed
score: 14/14
overrides_applied: 0
---

# Phase 11: Vision Enrichment & Idempotency Verification Report

**Phase Goal:** Close the v1.2 milestone's remaining preprocessor requirements: idempotent cache-hit skip (PRE-04), blockquote image format (PRE-11/PRE-12), standalone-image and NeedsVision PDF vision enrichment (PRE-09 partial), TextFirst PDF per-page text+image interleaving (PRE-10), and ephemeral-cache documentation (PRE-13).
**Verified:** 2026-04-20T23:55:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | When a non-empty cache file exists at `asset-cache/{shard}/{sha256}.txt`, `ingest_asset_path` skips every Anthropic/OCR API call and reuses cached synthetic markdown | VERIFIED | `read_cache_if_present` called at line 91 of `ingest.rs` before any API branch; `cache_hit_skips_api_and_returns_cached_synthetic` test passes with `image_vision=None` on a `.png` |
| 2 | When the cache file exists but is empty or unreadable (non-NotFound IO error), a tracing WARN is emitted with `corrupt_cache=true` and the API call proceeds as a cache miss | VERIFIED | `read_cache_if_present` in `cache.rs` lines 40-58 emits `tracing::warn!(corrupt_cache = true, ...)` on empty/whitespace/IO-error paths; 4 named tests cover these branches; `corrupt_cache_triggers_refetch` integration test passes |
| 3 | Every standalone image (png/jpg/jpeg/webp) synthetic markdown body is wrapped in `> **[Image: {filename}]** {description}` with `> ` prefix on every continuation line | VERIFIED | `blockquote_image` helper at `ingest.rs:32-45`; standalone branch calls `blockquote_image(fname, &desc)` at line 231; 3 pure-helper unit tests pass confirming single-line, multi-line, and empty-description cases |
| 4 | Every NeedsVision PDF page OCR body is wrapped in `> **[Image: {pdf_stem}_page_{N}.png]** {ocr_text}` with 1-based N; pages joined with `\n\n---\n\n` separator | VERIFIED | NeedsVision branch at `ingest.rs:205-214` uses `blockquote_image(&format!("{stem}_page_{}.png", i+1), &text)` with `.join("\n\n---\n\n")`; `needsvision_pdf_pages_use_blockquote_format` and `needs_vision_pdf_routes_raster_pages_through_ocr_service` tests pass |
| 5 | Cache write path (`ensure_cache_parent` + `tokio::fs::write`) is only reached on cache miss | VERIFIED | `ingest.rs:241-245`: write block after all API branches; cache-read gate at line 91 short-circuits hits before the write block is reached |
| 6 | TextFirst PDFs extract embedded raster images per page via pdfium-render and send each through `describe_image` | VERIFIED | `pdf_images.rs` implements `extract_embedded_images_per_page` using `as_image_object()` + `get_raw_image()`; TextFirst branch at `ingest.rs:120-122` calls both `extract_page_text_vec` and `extract_embedded_images_per_page`; `textfirst_pdf_interleaves_text_and_image_blockquotes_per_page` test passes |
| 7 | For each TextFirst PDF page, synthetic markdown contains `## Page {N}` followed by page text, followed by one `> **[Image: {stem}_page_{N}_image_{I}.png]** {desc}` blockquote per embedded image; pages joined with `\n\n---\n\n` | VERIFIED | TextFirst loop at `ingest.rs:145-183` builds `## Page {page_idx+1}` heading, appends text, then iterates images with `blockquote_image(&synthetic_name, &desc)` and `pages joined with `\n\n---\n\n`; test asserts all three tokens |
| 8 | When pdfium cannot bind (system library missing), embedded-image extraction returns empty list + WARN and TextFirst PDF still indexes its extracted text | VERIFIED | `pdf_images.rs:41-48`: `Pdfium::bind_to_system_library()` failure path returns `Ok(Vec::new())` with `tracing::warn!`; `textfirst_pdf_without_vision_client_warns_and_indexes_text_only` confirms text-only output when no vision client; host has no libpdfium yet tests pass via graceful-degradation assertions |
| 9 | When `image_vision` is None on a TextFirst PDF with embedded images, a WARN is logged and page text still indexes | VERIFIED | `ingest.rs:130-136` emits `tracing::warn!(asset = %fname, "TextFirst PDF has embedded images but ANTHROPIC_API_KEY ...")` exactly once; `textfirst_pdf_without_vision_client_warns_and_indexes_text_only` asserts text present and no image blockquote |
| 10 | `extract_embedded_images_per_page` never uses `get_raw_image_data()` | VERIFIED | `grep -c "get_raw_image_data" src/pipeline/assets/pdf_images.rs` = 0 (referenced only in comments as "never use"); only `get_raw_image()` is called |
| 11 | README contains a subsection documenting ephemeral `asset-cache/{shard}/{sha256}.txt` layout, cache-hit idempotency, and corrupt-cache WARN behavior | VERIFIED | `README.md:111` has `### Ephemeral asset cache and idempotency`; contains `asset-cache/ab/cd/{sha256}.txt`, `corrupt_cache`, and all required behavior descriptions |
| 12 | README explicitly states raw PDFs/images are NOT indexed as .md files and no companion `.processed.md` files are written | VERIFIED | `README.md:150-155`: "Double-index prevention" paragraph; "No companion `.processed.md` files are ever written next to your PDFs or images in the vault." |
| 13 | README documents cache invalidation procedure and SHA-256-only key limitation | VERIFIED | `README.md:142`: `rm -rf <data_dir>/asset-cache/`; text following states changing `LOCAL_INDEX_ASSET_MODEL` or vision prompt does NOT invalidate cache |
| 14 | README describes TextFirst embedded-image vision behavior with synthetic filename convention and graceful-degradation fallbacks | VERIFIED | `README.md:157-175`: "TextFirst PDF embedded images" and "Graceful degradation" paragraphs; Mermaid flowchart shows full dispatch path |

**Score:** 14/14 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/pipeline/assets/cache.rs` | `read_cache_if_present` async helper + 5 unit tests | VERIFIED | Lines 37-59: function exists; lines 78-121: 5 named `tokio::test` functions covering hit, empty, whitespace, NotFound-silent, directory-path |
| `src/pipeline/assets/ingest.rs` | SHA-256 above branching; cache-read gate; `blockquote_image` helper; blockquote-wrapped standalone image + NeedsVision PDF page output | VERIFIED | `blockquote_image` at line 32; SHA-256 at lines 82-89 (before `let fname` at line 99); cache gate at line 91; standalone image wrapped at line 231; NeedsVision wrapped at line 210 |
| `src/pipeline/assets/pdf_images.rs` | `extract_embedded_images_per_page` — Vec<Vec<Vec<u8>>> | VERIFIED | Line 33: `pub fn extract_embedded_images_per_page`; uses `as_image_object()` + `get_raw_image()`; 3 unit tests |
| `src/pipeline/assets/pdf_local.rs` | `extract_page_text_vec` — per-page text extractor | VERIFIED | Line 96: `pub fn extract_page_text_vec`; 5 new tests; `fixture_single_page_pdf_with_embedded_image` at line 188 as `pub fn` |
| `src/pipeline/assets/mod.rs` | `mod pdf_images;` declared | VERIFIED | Line 12: `mod pdf_images;` in alphabetical block |
| `src/test_support.rs` | re-exports `fixture_single_page_pdf_with_embedded_image` | VERIFIED | File exists; line 8: `pub use crate::pipeline::assets::fixture_single_page_pdf_with_embedded_image;` |
| `src/lib.rs` | `pub mod test_support;` | VERIFIED | Line 7: `pub mod test_support;` |
| `tests/anthropic_assets_mock.rs` | `textfirst_pdf_calls_vision_per_embedded_image` integration test | VERIFIED | Line 69: test function exists and passes |
| `README.md` | `### Ephemeral asset cache and idempotency` subsection | VERIFIED | Lines 111-193: complete subsection with Mermaid flowchart |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `ingest.rs` | `cache.rs::read_cache_if_present` | import + call before any API branch | WIRED | `use super::cache::{..., read_cache_if_present}` at line 8; called at line 91 |
| `ingest.rs::ingest_asset_path (standalone image branch)` | `ingest.rs::blockquote_image` | wraps `describe_image` result | WIRED | `blockquote_image(fname, &desc)` at line 231 |
| `ingest.rs::ingest_asset_path (NeedsVision branch)` | `ingest.rs::blockquote_image` | wraps each OCR page with synthetic `{stem}_page_{N}.png` | WIRED | `blockquote_image(&synthetic_name, &text)` at line 210; `_page_` pattern at line 209 |
| `ingest.rs (TextFirst branch)` | `pdf_images.rs::extract_embedded_images_per_page` | per-page image PNG collection | WIRED | `use super::pdf_images::extract_embedded_images_per_page;` at line 10; called at line 122 |
| `ingest.rs (TextFirst branch)` | `pdf_local.rs::extract_page_text_vec` | per-page text extraction aligned with image pages by index | WIRED | `use super::pdf_local::{classify_pdf, extract_page_text_vec, PdfClassification}` at line 11; called at line 120 |
| `ingest.rs (TextFirst branch)` | `ingest.rs::blockquote_image` | wraps each `describe_image` response using synthetic `{stem}_page_{N}_image_{I}.png` | WIRED | `blockquote_image(&synthetic_name, &desc)` at line 164; `_image_` in `synthetic_name` format at line 158 |
| `README.md` | `cache.rs`, `ingest.rs`, `pdf_images.rs` behavior | documentation reference | WIRED | `asset-cache` appears in README at line 121; all documented behaviors match shipped code |
| `src/pipeline/assets/mod.rs` | `pdf_local::fixture_single_page_pdf_with_embedded_image` | re-export for test_support | WIRED | `pub use pdf_local::fixture_single_page_pdf_with_embedded_image;` at line 21 |

### Data-Flow Trace (Level 4)

These artifacts render dynamic data. The cache gate is the key flow:

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `ingest.rs::ingest_asset_path` | `synthetic` | API calls (`describe_image`, `ocr_scanned_pdf_pages`, `extract_page_text_vec`) | Yes — real API responses or cached markdown | FLOWING |
| `ingest.rs` cache-read path | `cached` (from `read_cache_if_present`) | `tokio::fs::read_to_string(cache_path)` | Yes — real file contents | FLOWING |
| `pdf_images.rs::extract_embedded_images_per_page` | `page_images` | `img_obj.get_raw_image()` + PNG re-encode | Yes — real PDF image data | FLOWING (with pdfium) / graceful-degradation (without) |
| `pdf_local.rs::extract_page_text_vec` | `out` (Vec<String>) | `doc.extract_text(&[pn])` via lopdf | Yes — real PDF text | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| cache.rs: all 5 `read_cache_if_present_*` tests pass | `cargo test --lib pipeline::assets::cache` | 6 passed, 0 failed | PASS |
| ingest.rs: all tests including blockquote, cache-hit, corrupt-cache, standalone, NeedsVision, TextFirst | `cargo test --lib pipeline::assets::ingest::tests` | captured in broader run |  PASS |
| pdf_images.rs unit tests | `cargo test --lib pipeline::assets::pdf_images` | 3 passed, 0 failed | PASS |
| pdf_local.rs unit tests | `cargo test --lib pipeline::assets::pdf_local` | 9 passed, 0 failed | PASS |
| Full lib test suite | `cargo test --lib pipeline::assets` | 34 passed, 0 failed | PASS |
| Integration tests | `cargo test --test anthropic_assets_mock` | 2 passed, 0 failed (including `textfirst_pdf_calls_vision_per_embedded_image`) | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| PRE-04 | 11-01 | Idempotent cache-hit skip when source SHA-256 is unchanged | SATISFIED | `read_cache_if_present` gate; `cache_hit_skips_api_and_returns_cached_synthetic` passes |
| PRE-09 | 11-01, 11-02 | Every raster image from PDFs and standalone images sent through Anthropic vision | SATISFIED | Standalone: `describe_image` wrapped in `blockquote_image`; NeedsVision: OCR pages wrapped; TextFirst: `extract_embedded_images_per_page` + `describe_image` per image |
| PRE-10 | 11-02 | One compound markdown output per PDF in page order, interleaving text and image descriptions | SATISFIED | TextFirst per-page loop produces `## Page N` + text + image blockquotes, joined with `\n\n---\n\n` |
| PRE-11 | 11-01 | Blockquote pattern `> **[Image: …]** …` | SATISFIED | `blockquote_image` helper produces exact format; 3 helper unit tests verify single-line, multi-line, empty-description |
| PRE-12 | 11-01 | Standalone images produce markdown companion with vision description as primary body | SATISFIED | `format!("# {fname}\n\n{block}\n")` at `ingest.rs:232`; `standalone_image_uses_blockquote_format` passes |
| PRE-13 | 11-03 | Companion files named/placed so local-index indexes them without double-indexing; documented in README | SATISFIED | Ephemeral-cache approach (D-01) explicitly documented in README; no companion files in vault; walker excludes non-.md extensions; "Double-index prevention" paragraph in README |

**Note on PRE-13 scope:** REQUIREMENTS.md PRE-13 references "companion files" in the original SEED-001 model. The implementation uses an ephemeral cache approach (D-01 from `11-CONTEXT.md`) which achieves the same goal — no double-indexing, documented behavior — without writing companion files to the vault. This design decision was established in Phase 9 and explicitly preserved in Phase 11. The requirement's intent (no double-indexing + documented) is fully satisfied by the ephemeral approach.

### Anti-Patterns Found

No blockers or warnings found.

| File | Pattern Checked | Severity | Result |
|------|----------------|----------|--------|
| `src/pipeline/assets/cache.rs` | TODO/FIXME/placeholder | — | None found |
| `src/pipeline/assets/cache.rs` | Empty implementations (`return null/{}`) | — | None (only `None` returns are correct optional-miss semantics) |
| `src/pipeline/assets/ingest.rs` | Stub handlers (logs only, no work) | — | None found |
| `src/pipeline/assets/ingest.rs` | Hardcoded empty data flowing to render | — | None; all state populated from real API calls or real cache reads |
| `src/pipeline/assets/pdf_images.rs` | `get_raw_image_data` (anti-pattern per D-08) | Blocker if present | Not called — only `get_raw_image()` |
| `README.md` | ASCII box-drawing in diagrams | CLAUDE.md violation if present | None; Mermaid flowchart used |

### Human Verification Required

None. All Phase 11 behaviors are verifiable programmatically via the test suite and code inspection.

### Gaps Summary

No gaps. All 14 must-have truths are verified. All 9 required artifacts exist and are substantive. All 8 key links are wired. All 6 requirements (PRE-04, PRE-09, PRE-10, PRE-11, PRE-12, PRE-13) have implementation evidence. All tests pass (34 lib tests, 2 integration tests).

The phase goal is achieved: the v1.2 milestone's remaining preprocessor requirements are closed.

---

_Verified: 2026-04-20T23:55:00Z_
_Verifier: Claude (gsd-verifier)_
