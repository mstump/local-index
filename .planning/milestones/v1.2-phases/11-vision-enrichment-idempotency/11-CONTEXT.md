# Phase 11: Vision enrichment & idempotency - Context

**Gathered:** 2026-04-20
**Status:** Ready for planning

<domain>

## Phase Boundary

Phase 11 delivers **PRE-04, PRE-09–PRE-12, PRE-13 completion** within the ephemeral-cache architecture established in Phase 9:

- **No companion files in the vault.** Phase 9 D-01/D-02/D-04 is preserved: intermediate text stays under `.local-index/asset-cache/`; chunks in LanceDB use the original asset path as `file_path`.
- **Idempotency** (PRE-04): source SHA-256 cache-hit check skips API calls when an asset is unchanged.
- **Blockquote format** (PRE-11): all image descriptions — standalone and PDF — use `> **[Image: filename]** desc`.
- **TextFirst PDF embedded-image vision** (PRE-10 completion): natively extract embedded images from TextFirst PDF pages; call Anthropic vision on any page with ≥1 extracted image; interleave descriptions with extracted text.
- **PRE-13 completion**: README documentation update only (walker-level double-index prevention already shipped in Phase 9).

**Explicitly out of scope:** Companion `.processed.md` files in the vault, new LanceDB schema columns (no `source_hash` column added), changes to the OCR provider abstraction (Phase 10), Google Document AI changes.

</domain>

<decisions>

## Implementation Decisions

### Output model

- **D-01:** **Stay ephemeral.** No companion `.processed.md` files are written into the vault. Phase 9 D-01/D-02/D-04 remains locked. Image descriptions live only in the in-memory synthetic markdown and in the `.local-index/asset-cache/` cache files.

### Idempotency (PRE-04)

- **D-02:** Before calling any API (Anthropic vision or OCR), compute the source file's SHA-256. If `asset-cache/{shard}/{sha256}.txt` already exists and is non-empty, read synthetic markdown from cache — skip the API call entirely. The existing LanceDB chunk `content_hash` check handles the re-embed skip as usual (no schema changes required).
- **D-03:** If the cache file exists but is corrupt (read error, empty file, partial write): log a `WARN` tracing event (e.g., `corrupt_cache = true, path = ...`) and re-fetch from the API, treating it as a cache miss. Overwrite the cache on successful re-fetch.

### Image description format (PRE-11)

- **D-04:** Apply the blockquote format to **all** image descriptions — both standalone raster images and rasterized PDF pages from the NeedsVision path.
- **D-05:** The label inside the blockquote is the **filename only** (not the full vault-relative path). Format: `> **[Image: figure_1.png]** <description>`.
- **D-06:** NeedsVision PDF pages continue to use `---` (markdown horizontal rule) as the separator between pages — existing behavior, no change.

### TextFirst PDF embedded-image vision (PRE-10)

- **D-07:** TextFirst PDFs now also receive embedded image extraction + vision. For each TextFirst PDF page, use pdfium's native image extraction API (not full-page rasterization of text pages) to pull embedded image objects. Pages with ≥1 extracted image get Anthropic vision called on each image; descriptions are interleaved with the extracted text in page order.
- **D-08:** The extraction mechanism is **native pdfium image extraction** (not rasterizing entire text pages). Researcher/planner verifies which pdfium-sys / pdfium-render API surface is available and how to extract embedded images per page.
- **D-09:** Vision is called on **all pages that yield ≥1 extracted image object** — no size/area threshold filtering. If the PDF has an embedded image on a page, it gets described.

### PRE-13 completion

- **D-10:** No new code needed for double-index prevention (Phase 9 shipped the walker exclusion + `prune_absent_markdown_files` guard). Phase 11 delivers the README documentation describing the ephemeral-cache approach and how operators avoid indexing raw PDFs/images.

### Claude's Discretion

- How to interleave extracted text paragraphs and image blockquotes within a page (before text? after? adjacent to image position?) — planner decides within D-07/D-08.
- Whether pdfium's image extraction returns positional metadata that enables inline interleaving or just a list of images per page — researcher confirms.
- Exact WARN log field names for D-03 — follow existing tracing field patterns in the codebase.

</decisions>

<canonical_refs>

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements

- `.planning/REQUIREMENTS.md` — PRE-04, PRE-09, PRE-10, PRE-11, PRE-12, PRE-13 definitions
- `.planning/ROADMAP.md` — Phase 11 goal and success criteria

### Prior phase context (MUST read for architecture constraints)

- `.planning/phases/09-preprocessor-foundation/09-CONTEXT.md` — D-01/D-02/D-04: ephemeral cache, asset `file_path`, no companion files
- `.planning/phases/09-preprocessor-foundation/09-RESEARCH.md` — rasterization patterns, pdfium usage
- `.planning/phases/10-ocr-providers/10-CONTEXT.md` — OCR provider abstraction (Phase 10 scope split)

### Seed

- `.planning/seeds/SEED-001-pdf-image-processor-daemon.md` — historical companion-file model (blockquote convention is adopted; vault-level companion output is NOT adopted per Phase 9 D-04)

### Code integration points

- `src/pipeline/assets/ingest.rs` — `ingest_asset_path()`: primary site for cache read-before-write (D-02/D-03) and blockquote format (D-04/D-05)
- `src/pipeline/assets/cache.rs` — `cache_path_for_hash()`, `cache_dir()`: existing cache helpers to extend with read logic
- `src/pipeline/assets/pdf_local.rs` — TextFirst classification and text extraction; Phase 11 adds per-page image extraction call from this path
- `src/pipeline/assets/pdf_raster.rs` — rasterization (NeedsVision path); review pdfium API surface for image extraction (D-08)
- `src/pipeline/assets/anthropic_extract.rs` — `describe_image()`: existing vision method; will be called per embedded image (D-07)

</canonical_refs>

<code_context>

## Existing Code Insights

### Reusable Assets

- `cache_path_for_hash(data_dir, sha256_hex)` — already computes the correct cache path; extend `ingest_asset_path` to check if it exists before calling API
- `AnthropicAssetClient::describe_image(media_type, bytes)` — existing vision method; reuse for embedded PDF images in TextFirst path
- `rasterize_pdf_pages_to_png()` — NeedsVision rasterization; review whether pdfium-render exposes image extraction separately

### Established Patterns

- Source SHA-256 is already computed in `ingest_asset_path` (bottom of function for cache write); move the hash computation to the top so it gates the API call
- Tracing WARN pattern: `tracing::warn!(field = %value, "message")` — follow existing style in `ingest.rs`
- `---` page separator already used for NeedsVision pages in `ingest_asset_path`

### Integration Points

- `ingest_asset_path` is the single insertion point for all changes — cache read, blockquote wrapping, and embedded image extraction all go here or in helpers it calls
- TextFirst branch currently: `extract_text_pdf_as_markdown(&bytes, max_bytes)?` → must become a per-page loop that also extracts and describes embedded images

</code_context>

<specifics>

## Specific Ideas

- Blockquote example from SEED-001: `> **[Image: figure_1.png]** Description of the semantic content`
- Cache read should happen before any file processing — compute hash from `bytes`, check cache, branch before the `if ext == "pdf"` block
- TextFirst PDFs currently produce flat text; Phase 11 should produce richer content with image blockquotes interleaved, but the chunker/embedder pipeline is unchanged

</specifics>

<deferred>

## Deferred Ideas

- Size/area threshold for embedded image filtering (e.g., skip logos < 50px) — not added in Phase 11; all images with ≥1 object get described
- LanceDB `source_hash` column for source-level idempotency (separate from chunk `content_hash`) — not needed if cache-based idempotency covers the use case
- TextFirst PDF rasterization approach as fallback if native extraction is insufficient — deferred; researcher confirms pdfium API first

</deferred>

---

*Phase: 11-vision-enrichment-idempotency*
*Context gathered: 2026-04-20*
