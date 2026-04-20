# Phase 11: Vision enrichment & idempotency — Research

**Researched:** 2026-04-20
**Domain:** PDF image extraction (pdfium-render), Anthropic vision, SHA-256 cache idempotency
**Confidence:** HIGH (pdfium-render API, Anthropic vision, existing code surface); MEDIUM (exact interleave ordering heuristic), LOW-flagged items called out explicitly

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Output model**

- **D-01:** **Stay ephemeral.** No companion `.processed.md` files are written into the vault. Phase 9 D-01/D-02/D-04 remains locked. Image descriptions live only in the in-memory synthetic markdown and in the `.local-index/asset-cache/` cache files.

**Idempotency (PRE-04)**

- **D-02:** Before calling any API (Anthropic vision or OCR), compute the source file's SHA-256. If `asset-cache/{shard}/{sha256}.txt` already exists and is non-empty, read synthetic markdown from cache — skip the API call entirely. The existing LanceDB chunk `content_hash` check handles the re-embed skip as usual (no schema changes required).
- **D-03:** If the cache file exists but is corrupt (read error, empty file, partial write): log a `WARN` tracing event (e.g., `corrupt_cache = true, path = ...`) and re-fetch from the API, treating it as a cache miss. Overwrite the cache on successful re-fetch.

**Image description format (PRE-11)**

- **D-04:** Apply the blockquote format to **all** image descriptions — both standalone raster images and rasterized PDF pages from the NeedsVision path.
- **D-05:** The label inside the blockquote is the **filename only** (not the full vault-relative path). Format: `> **[Image: figure_1.png]** <description>`.
- **D-06:** NeedsVision PDF pages continue to use `---` (markdown horizontal rule) as the separator between pages — existing behavior, no change.

**TextFirst PDF embedded-image vision (PRE-10)**

- **D-07:** TextFirst PDFs now also receive embedded image extraction + vision. For each TextFirst PDF page, use pdfium's native image extraction API (not full-page rasterization of text pages) to pull embedded image objects. Pages with ≥1 extracted image get Anthropic vision called on each image; descriptions are interleaved with the extracted text in page order.
- **D-08:** The extraction mechanism is **native pdfium image extraction** (not rasterizing entire text pages). Researcher/planner verifies which pdfium-sys / pdfium-render API surface is available and how to extract embedded images per page.
- **D-09:** Vision is called on **all pages that yield ≥1 extracted image object** — no size/area threshold filtering. If the PDF has an embedded image on a page, it gets described.

**PRE-13 completion**

- **D-10:** No new code needed for double-index prevention (Phase 9 shipped the walker exclusion + `prune_absent_markdown_files` guard). Phase 11 delivers the README documentation describing the ephemeral-cache approach and how operators avoid indexing raw PDFs/images.

### Claude's Discretion

- How to interleave extracted text paragraphs and image blockquotes within a page (before text? after? adjacent to image position?) — planner decides within D-07/D-08.
- Whether pdfium's image extraction returns positional metadata that enables inline interleaving or just a list of images per page — researcher confirms.
- Exact WARN log field names for D-03 — follow existing tracing field patterns in the codebase.

### Deferred Ideas (OUT OF SCOPE)

- Size/area threshold for embedded image filtering (e.g., skip logos < 50px) — not added in Phase 11; all images with ≥1 object get described.
- LanceDB `source_hash` column for source-level idempotency (separate from chunk `content_hash`) — not needed if cache-based idempotency covers the use case.
- TextFirst PDF rasterization approach as fallback if native extraction is insufficient — deferred; researcher confirms pdfium API first.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PRE-04 | When a companion file already exists and its YAML frontmatter source content hash matches the current source file SHA-256, processing is skipped (idempotent) | "Cache-backed idempotency" section — reuse existing `cache_path_for_hash`; read before write in `ingest_asset_path`. Note: CONTEXT D-01 replaces "companion frontmatter hash" with "cache file existence keyed by source SHA-256" — functional equivalent, no frontmatter needed. |
| PRE-09 | Every raster image from PDFs and every standalone image file is sent through Anthropic vision for semantic description | "Embedded image extraction" section — per-page pdfium `objects().iter()` filter `as_image_object()`; reuse `AnthropicAssetClient::describe_image` (existing). |
| PRE-10 | For each PDF, output is one companion markdown file with body content in page order, interleaving extracted text and image descriptions | "Interleaving strategy" section — build synthetic markdown per page, concatenate with `---` separators (D-06). In-memory only (D-01). |
| PRE-11 | Image descriptions use the blockquote pattern from SEED-001 (e.g. `> **[Image: …]** …`) | "Blockquote formatter" section — helper function, filename-only label per D-05. |
| PRE-12 | Standalone images produce a small markdown companion with the vision description as the primary body | "Standalone image output" section — wrap the existing `describe_image` result in the blockquote format. |
| PRE-13 | Companion files are named and placed so local-index indexes them as normal `.md` without indexing raw PDFs/images twice; convention is documented in README | "README update" section — Phase 9 already excludes raw `.pdf`/image extensions from the markdown walker; Phase 11 closes this requirement with documentation only (D-10). |
</phase_requirements>

## Summary

Phase 11 closes the v1.2 milestone by adding two capabilities to the existing asset pipeline: **(1)** cache-based idempotency that short-circuits Anthropic API calls when a source asset's SHA-256 matches an existing cache entry, and **(2)** embedded-image extraction for TextFirst PDFs via `pdfium-render`, so text pages with figures are also sent through Anthropic vision — with descriptions interleaved into the per-page synthetic markdown. The blockquote format `> **[Image: filename.png]** <desc>` is applied uniformly across standalone images, NeedsVision PDF pages, and newly-extracted TextFirst embedded images.

The investigation confirms the pdfium-render API surface is sufficient and idiomatic: `PdfPages.iter()` → `PdfPage.objects().iter()` → `PdfPageObject.as_image_object()` → `PdfPageImageObject.get_raw_image()` returns a decoded `image::DynamicImage` that can be re-encoded to PNG for Anthropic vision (avoiding the filter-chain landmines of `get_raw_image_data()`). Page reading order is derivable from object bounds (`PdfPageObjectCommon::bounds()` returning `PdfQuadPoints` with `top()`/`bottom()`/`left()`/`right()` helpers), but for the Phase 11 scope a simpler "text first, then blockquote per image in iteration order" or "image blockquotes last, after full page text" ordering is the least surprising and easiest to verify. The extracted-text approach of `extract_text_pdf_as_markdown` uses `lopdf` (does not return per-text-object positions); switching to pdfium's `PdfPageTextObject` would be required if true inline positional interleaving is needed — but that's bigger scope than PRE-10 requires.

Most Phase 11 complexity is code-structural (moving hash computation to the top of `ingest_asset_path`, adding a per-page loop for TextFirst PDFs) rather than algorithmic. No new dependencies are required.

**Primary recommendation:** Add SHA-256 cache-read gating at the top of `ingest_asset_path` (one code path, covers all asset types). Refactor the TextFirst PDF branch into a per-page loop using `pdfium-render` for both text objects (via `PdfPageTextObject::text()`) and image objects (via `PdfPageImageObject::get_raw_image()` → re-encode PNG → `describe_image`), emitting synthetic markdown of the form: `# Source: {filename}\n\n## Page {n}\n\n{text}\n\n> **[Image: {page_n_image_i}]** {desc}\n\n---\n\n...`. Standalone images use the same blockquote wrapper. Retain `lopdf`-based classification (`classify_pdf`) and the existing `---` page separator convention. Keep NeedsVision OCR pages unchanged in structure but wrap per-page output in the blockquote format.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Source-hash idempotency check | Asset pipeline (`src/pipeline/assets/ingest.rs`) | Cache helper (`src/pipeline/assets/cache.rs`) | Single orchestration point gates all API calls (PDF + image); cache module owns path/IO primitives. |
| TextFirst embedded image extraction | PDF subsystem (`src/pipeline/assets/pdf_local.rs` or new `pdf_images.rs`) | — | Keeps classification + extraction under one PDF module; pdfium-render already linked via `pdf_raster.rs`. |
| Vision description per embedded image | Anthropic client (`src/pipeline/assets/anthropic_extract.rs`) | Asset pipeline (orchestration) | Reuses `describe_image(mt, bytes)`; no new HTTP code. |
| Page reassembly / blockquote formatting | Asset pipeline (`ingest.rs`) | Small formatter helper | Pure string composition; no new module required. |
| Standalone image output format | Asset pipeline (`ingest.rs`) | — | Already the owner of per-extension synthetic markdown. |
| Cache write/read | Cache module (`cache.rs`) | Asset pipeline (orchestrator) | Paths/sharding already live in `cache.rs`; add a `read_cache_if_present` helper next to existing `cache_path_for_hash`. |
| README documentation (PRE-13) | Docs (`README.md`) | — | No code; Phase 9 already shipped the double-index guard. |

## Standard Stack

### Core (all already in Cargo.toml — no new deps)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pdfium-render` | `0.8.37` | PDF text + image object extraction for TextFirst PDFs | Already used for rasterization in `pdf_raster.rs`. Exposes `PdfPageImageObject::get_raw_image() -> DynamicImage` (added pre-0.8.30) and `get_raw_image_data() -> Vec<u8>` (added 0.8.37). `[VERIFIED: Cargo.lock]` |
| `lopdf` | `0.38` | PDF classification (existing) | Already used in `pdf_local.rs::classify_pdf`. Keep for classification — no need to migrate to pdfium-render for heuristic. `[VERIFIED: Cargo.toml]` |
| `image` | `=0.25.4` (png feature) | Re-encode `DynamicImage` → PNG bytes for Anthropic vision | Already a dependency pinned at `=0.25.4` with `png` feature. `DynamicImage::write_to(&mut Cursor, ImageFormat::Png)` pattern already used in `pdf_raster.rs::try_pdfium`. `[VERIFIED: Cargo.toml]` |
| `sha2` | `0.11` | SHA-256 of source bytes for cache key | Already used in `ingest.rs` — just move the computation earlier in the function. `[VERIFIED: Cargo.toml]` |
| `tokio::fs` | (tokio 1.40) | Async read/write of cache file | Already used for cache write. Phase 11 adds read-before-write. `[VERIFIED: existing code]` |
| `reqwest` / Anthropic client | existing | `describe_image(media_type, bytes)` for each embedded image | `AnthropicAssetClient::describe_image` is already public and generic over media type. `[VERIFIED: src/pipeline/assets/anthropic_extract.rs:69]` |
| `tracing` | `0.1` | WARN log for corrupt cache (D-03) | Pattern `tracing::warn!(field = %value, "message")` already established. `[VERIFIED: src/pipeline/assets/ingest.rs:125-127]` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `PdfPageImageObject::get_raw_image()` (decoded `DynamicImage`) | `get_raw_image_data()` (raw embedded bytes) | `get_raw_image_data()` returns the compressed stream exactly as embedded in the PDF — this may be DCTDecode (JPEG), JPXDecode (JPEG2000), FlateDecode (raw pixel data in zlib wrapper), CCITTFaxDecode (fax-style monochrome), or a filter chain. Anthropic vision accepts only `image/jpeg`, `image/png`, `image/gif`, `image/webp`. Since we cannot safely infer the media type without parsing filters, **use `get_raw_image()`** — it returns a decoded `DynamicImage` that we re-encode to PNG. The cost is one extra decode/encode round-trip per image, which is negligible vs Anthropic latency. `[CITED: https://docs.rs/pdfium-render/0.8.37/pdfium_render/prelude/struct.PdfPageImageObject.html]` |
| `lopdf`-based text extraction (current `extract_text_pdf_as_markdown`) | pdfium-render `PdfPageTextObject::text()` per page | Current code returns one flat string per page via `lopdf`. Switching to pdfium-render gives per-object text + bounds, enabling true spatial interleaving. **Tradeoff:** bigger refactor, and Phase 11 can ship a simpler "text first, then image blockquotes" page layout that satisfies PRE-10 without per-object text parsing. Recommend keeping `lopdf` for per-page text and only using pdfium-render for image extraction on the same PDF bytes. |
| Positional interleaving (sort text + images by bounds.top() descending then left() ascending) | Simpler "text then blockquotes" layout | PDF coordinates are **origin bottom-left**, so visual "top" = higher Y. `PdfQuadPoints` exposes `top()`/`bottom()`/`left()`/`right()` helpers. True positional interleaving of text from lopdf + images from pdfium-render requires reconciling two coordinate systems. Recommend Phase 11 uses the simpler layout; true positional interleaving is a follow-up. `[CITED: https://docs.rs/pdfium-render/0.8.37/pdfium_render/prelude/struct.PdfQuadPoints.html]` |
| Adding `content_hash` YAML frontmatter (SEED-001 model) | Cache-file-existence idempotency (D-01, D-02) | Per CONTEXT D-01 the vault contains no companion files; therefore "frontmatter source hash" has no file to live in. The existing `asset-cache/{shard}/{sha256}.txt` layout is functionally equivalent: presence of a non-empty cache file keyed by the source SHA-256 means "we've already processed this exact byte content." |

**No new installs required.**

### Version verification

All Phase 11 dependencies are already in `Cargo.toml` and the workspace compiles against them:

- `pdfium-render = "0.8.37"` — `[VERIFIED: /Users/matthewstump/src/local-index/Cargo.toml:97]` and `[VERIFIED: Cargo.lock]`
- `lopdf = "0.38"` — `[VERIFIED: Cargo.toml:96]`
- `image = "=0.25.4"` with `png` feature — `[VERIFIED: Cargo.toml:98]`
- `sha2 = "0.11"` — `[VERIFIED: Cargo.toml]`

## Architecture Patterns

### System Architecture Diagram

```mermaid
flowchart TD
    IN[ingest_asset_path &#40;asset_rel, data_dir, ocr, vision&#41;] --> READ[read source bytes]
    READ --> SZ{bytes &gt; max_bytes?}
    SZ -->|yes| ERR[AssetTooLarge error]
    SZ -->|no| HASH[compute SHA-256 of bytes]
    HASH --> CACHE{cache hit at<br/>asset-cache/ab/cd/&#123;sha256&#125;.txt?}
    CACHE -->|hit, non-empty| READCACHE[read cache file]
    READCACHE --> CHUNK[chunk_markdown synthetic, asset_rel]
    CACHE -->|hit, empty/corrupt| WARN[tracing::warn!]
    WARN --> MISS
    CACHE -->|miss| MISS[branch by extension]
    MISS -->|pdf| CLASSIFY[classify_pdf via lopdf]
    MISS -->|png/jpg/webp| IMG[describe_image mt, bytes]
    CLASSIFY -->|TextFirst| TF[TextFirst per-page loop]
    CLASSIFY -->|NeedsVision| NV[rasterize + OcrService]
    TF --> PDFIUM[pdfium-render: iter pages]
    PDFIUM --> PAGE[for each page:]
    PAGE --> TEXT[extract page text lopdf]
    PAGE --> IMGS[iter objects, filter as_image_object]
    IMGS --> DEC[get_raw_image -&gt; DynamicImage]
    DEC --> PNG[encode PNG via image crate]
    PNG --> VISION[describe_image image/png, bytes]
    VISION --> BLOCK[blockquote format: &gt; **[Image: filename.png]** desc]
    TEXT --> COMPOSE[compose page: text + blockquotes]
    BLOCK --> COMPOSE
    COMPOSE --> JOIN[join pages with ---]
    NV --> NVOCR[OcrService.ocr_scanned_pdf_pages]
    NVOCR --> NVBLOCK[blockquote per page: &gt; **[Image: filename_page_N.png]** ocr_text]
    NVBLOCK --> NVJOIN[join with ---]
    IMG --> IMGBLOCK[blockquote: &gt; **[Image: filename.png]** desc]
    JOIN --> SYN[synthetic markdown]
    NVJOIN --> SYN
    IMGBLOCK --> SYN
    SYN --> WRITE[tokio::fs::write cache]
    WRITE --> CHUNK
    CHUNK --> OUT[ChunkedFile]
```

### Recommended Project Structure

```
src/pipeline/assets/
├── mod.rs                    # Re-exports (existing)
├── ingest.rs                 # EXTEND: add cache-read-before-API gate, per-page loop for TextFirst
├── cache.rs                  # EXTEND: add read_cache_if_present(path) -> Option<String>
├── pdf_local.rs              # KEEP classify_pdf + extract_text_pdf_as_markdown (lopdf)
├── pdf_raster.rs             # UNCHANGED: NeedsVision rasterization
├── pdf_images.rs             # NEW (optional): per-page embedded image extraction via pdfium-render
├── anthropic_extract.rs      # UNCHANGED: describe_image is already suitable
├── document_ai.rs            # UNCHANGED
├── ocr.rs                    # UNCHANGED
└── ignore_walk.rs            # UNCHANGED
```

The **new `pdf_images.rs` module is optional** — the extraction code is short enough that it can live in `ingest.rs` or `pdf_local.rs`. Recommendation: put it in a new `pdf_images.rs` to keep `ingest.rs` focused on orchestration.

### Pattern 1: Cache-read gate at the top of `ingest_asset_path`

**What:** Compute source SHA-256 immediately after the read, check the cache file, return early if hit.
**When to use:** Every asset path (PDF, PNG, JPG, WebP) — same gate for all.
**Example:**

```rust
// Source: synthesized from existing ingest.rs patterns + D-02/D-03
use sha2::{Digest, Sha256};

pub async fn ingest_asset_path(
    vault: &Path,
    asset_rel: &Path,
    data_dir: &Path,
    max_bytes: usize,
    max_pdf_pages: usize,
    pdf_ocr: Option<&OcrService>,
    image_vision: Option<&AnthropicAssetClient>,
) -> Result<ChunkedFile, LocalIndexError> {
    // ... existing path canonicalization + size check ...

    let bytes = tokio::fs::read(&abs).await.map_err(LocalIndexError::Io)?;
    // size check ...

    // === PHASE 11: cache-read gate (D-02, D-03) ===
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let cache_path = cache_path_for_hash(data_dir, &hex);

    if let Some(cached) = read_cache_if_present(&cache_path).await {
        tracing::debug!(path = %cache_path.display(), "asset cache hit; skipping API");
        return chunk_markdown(&cached, asset_rel);
    }
    // fall through: cache miss or corrupt — existing WARN inside read_cache_if_present

    // ... existing per-extension branches produce `synthetic: String` ...

    // Existing cache write (now always reached only on cache miss)
    ensure_cache_parent(&cache_path)?;
    tokio::fs::write(&cache_path, synthetic.as_bytes()).await?;

    chunk_markdown(&synthetic, asset_rel)
}
```

Where `read_cache_if_present` lives in `cache.rs`:

```rust
// Source: new helper in src/pipeline/assets/cache.rs
/// Read a cache file if present. Returns None on miss or corrupt cache
/// (empty file, IO error); logs a WARN for corruption per D-03.
pub async fn read_cache_if_present(path: &Path) -> Option<String> {
    match tokio::fs::read_to_string(path).await {
        Ok(s) if !s.trim().is_empty() => Some(s),
        Ok(_) => {
            tracing::warn!(
                corrupt_cache = true,
                path = %path.display(),
                "asset cache file exists but is empty; refetching from API"
            );
            None
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            tracing::warn!(
                corrupt_cache = true,
                path = %path.display(),
                error = %e,
                "asset cache read failed; refetching from API"
            );
            None
        }
    }
}
```

### Pattern 2: Per-page embedded image extraction (pdfium-render)

**What:** For a TextFirst PDF page, iterate `page.objects()` and collect each `PdfPageImageObject`'s decoded pixel data as PNG.
**When to use:** Inside the TextFirst branch, per page.
**Example:**

```rust
// Source: adapted from https://github.com/ajrcarey/pdfium-render/blob/master/examples/image_extract.rs
use std::io::Cursor;
use image::ImageFormat;
use pdfium_render::prelude::*;

/// Extract PNG byte buffers for all embedded images on each page of a PDF.
/// Returns `Vec<Vec<Vec<u8>>>` indexed by page -> image index -> PNG bytes.
pub fn extract_embedded_images_per_page(
    pdf_bytes: &[u8],
    max_pages: usize,
) -> Result<Vec<Vec<Vec<u8>>>, LocalIndexError> {
    let bindings = Pdfium::bind_to_system_library()
        .map_err(|e| LocalIndexError::Config(format!("pdfium: {e}")))?;
    let pdfium = Pdfium::new(bindings);
    let doc = pdfium
        .load_pdf_from_byte_slice(pdf_bytes, None)
        .map_err(|e| LocalIndexError::Config(format!("pdfium load: {e}")))?;

    let mut pages_out: Vec<Vec<Vec<u8>>> = Vec::new();
    for (page_idx, page) in doc.pages().iter().enumerate() {
        if page_idx >= max_pages {
            break;
        }
        let mut page_images = Vec::new();
        for object in page.objects().iter() {
            if let Some(img_obj) = object.as_image_object() {
                // get_raw_image() returns a decoded DynamicImage; safe for re-encode.
                // get_raw_image_data() would return filter-encoded bytes (DCTDecode
                // JPEG, JPXDecode, FlateDecode, CCITTFax) with no reliable way to
                // detect the media type — not suitable for Anthropic vision.
                if let Ok(dyn_image) = img_obj.get_raw_image() {
                    let mut buf = Vec::new();
                    if dyn_image
                        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
                        .is_ok()
                    {
                        page_images.push(buf);
                    }
                }
            }
        }
        pages_out.push(page_images);
    }
    Ok(pages_out)
}
```

Key API facts `[CITED: https://docs.rs/pdfium-render/0.8.37/pdfium_render/prelude/enum.PdfPageObject.html]`:

- `PdfPageObject::as_image_object()` returns `Option<&PdfPageImageObject>` — only `Some` for image variant.
- `PdfPageImageObject::get_raw_image()` returns `Result<image::DynamicImage, PdfiumError>` — gated by the `image` feature (on by default with pdfium-render).
- Re-encoding `DynamicImage → PNG` via `image::ImageFormat::Png` is already done in `pdf_raster.rs::try_pdfium`.

### Pattern 3: Blockquote formatting (D-04, D-05)

**What:** Wrap a single image description in the canonical blockquote.
**When to use:** Every image description, regardless of source (standalone, NeedsVision page, TextFirst embedded).
**Example:**

```rust
// Source: SEED-001 convention + CONTEXT D-04/D-05
/// Format a vision description as the canonical image blockquote.
/// Label is the filename only per D-05, not the vault-relative path.
fn blockquote_image(filename: &str, description: &str) -> String {
    // Multi-line descriptions: each continuation line also prefixed with "> "
    // so the blockquote is one contiguous block in markdown.
    let mut out = String::new();
    out.push_str(&format!("> **[Image: {filename}]** "));
    let mut first = true;
    for line in description.lines() {
        if first {
            out.push_str(line);
            first = false;
        } else {
            out.push_str("\n> ");
            out.push_str(line);
        }
    }
    out
}
```

**Filename derivation** per the per-image case:

- Standalone image: the asset's `file_name()` (e.g. `diagram.png`).
- NeedsVision PDF page N: synthetic `{pdf_stem}_page_{n}.png` (stable, informative; the image was rasterized from a page, not a real file on disk).
- TextFirst embedded image: synthetic `{pdf_stem}_page_{n}_image_{i}.png` (follows the pdfium example's naming).

### Pattern 4: Per-page composition for TextFirst PDFs (D-07, D-08)

**What:** Build synthetic markdown that preserves page order and includes image blockquotes.
**When to use:** TextFirst branch of `ingest_asset_path`.
**Example:**

```rust
// Source: synthesized; integrates patterns 2 and 3 with existing extract_text_pdf_as_markdown
async fn textfirst_pdf_with_images(
    bytes: &[u8],
    fname: &str,
    max_bytes: usize,
    max_pages: usize,
    vision: Option<&AnthropicAssetClient>,
) -> Result<String, LocalIndexError> {
    // 1) page-wise text via lopdf (existing)
    let per_page_text = extract_page_text_vec(bytes, max_bytes, max_pages)?;

    // 2) per-page image PNGs via pdfium-render (new)
    let per_page_images = extract_embedded_images_per_page(bytes, max_pages)?;

    let stem = Path::new(fname)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc");

    let mut page_chunks: Vec<String> = Vec::new();
    for (page_idx, text) in per_page_text.iter().enumerate() {
        let mut page = String::new();
        page.push_str(&format!("## Page {}\n\n", page_idx + 1));
        if !text.trim().is_empty() {
            page.push_str(text.trim());
            page.push_str("\n\n");
        }
        if let Some(images) = per_page_images.get(page_idx) {
            for (img_idx, png) in images.iter().enumerate() {
                let synthetic_name = format!(
                    "{stem}_page_{}_image_{}.png",
                    page_idx + 1,
                    img_idx + 1
                );
                // D-09: no size threshold filtering
                if let Some(client) = vision {
                    let desc = client
                        .describe_image("image/png", png)
                        .await
                        .unwrap_or_else(|e| format!("[vision error: {e}]"));
                    page.push_str(&blockquote_image(&synthetic_name, &desc));
                    page.push_str("\n\n");
                }
                // If vision is None, we still got images but have no describer.
                // Phase 9 policy: standalone images without vision return Credential error;
                // for TextFirst embedded images the page text still indexes, so we
                // degrade gracefully: skip this image's description.
            }
        }
        page_chunks.push(page.trim_end().to_string());
    }

    let body = page_chunks.join("\n\n---\n\n");
    Ok(format!("# Source: {fname}\n\n{body}"))
}
```

**Design note on `vision == None` for TextFirst PDFs:** The existing TextFirst branch currently does not require `image_vision` (it works on pure text). After Phase 11, if embedded images are present but `ANTHROPIC_API_KEY` is missing, the planner decides: **(a)** fail fast (consistent with standalone images today), or **(b)** degrade gracefully and omit image descriptions (preserves backward compat — TextFirst PDFs with no images keep working; TextFirst PDFs with images lose the image descriptions but still index text). Recommendation: **graceful degradation with a WARN** — aligns with `vision.ok()` fallback pattern already in `build_ocr_and_image_clients`.

### Anti-Patterns to Avoid

- **Using `get_raw_image_data()` and sending those bytes directly to Anthropic** — the raw bytes are whatever PDF filter chain was used (DCTDecode/JPXDecode/FlateDecode/CCITTFaxDecode). Anthropic vision rejects unsupported media types. Always re-encode through `image::DynamicImage` to PNG. `[CITED: https://docs.rs/pdfium-render/0.8.37/pdfium_render/prelude/struct.PdfPageImageObject.html]`
- **Running SHA-256 on streamed chunks instead of the whole buffer** — we already hold the full `Vec<u8>` in `bytes`; just hash it. Don't complicate with streaming.
- **Storing the cache file with trailing whitespace or a terminator** — `read_cache_if_present` uses `trim().is_empty()` to detect corruption; ensure writes don't silently produce all-whitespace content.
- **Blockquote label using the full vault-relative path** — D-05 locks the label to filename only.
- **Adding a new LanceDB column** — CONTEXT boundary explicitly forbids schema changes for Phase 11.
- **Writing `.processed.md` companion files into the vault** — CONTEXT D-01 locks ephemeral-only.
- **Sorting extracted objects by "document order" and assuming that's reading order** — for non-trivial layouts (newspapers, multi-column) this fails. For Phase 11's scope, the "text first, then all images per page as blockquotes" layout avoids this problem entirely.
- **Re-running pdfium load_pdf for both classification and extraction** — currently `classify_pdf` uses `lopdf::Document::load_mem`, and rasterization uses `pdfium::load_pdf_from_byte_slice`. Phase 11's image extraction reuses the pdfium load. Don't load_pdf three times; load once per branch.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| PDF image extraction | Parsing `/XObject` streams manually; decoding FlateDecode/DCTDecode/JPXDecode | `pdfium-render` `page.objects().iter().filter_map(|o| o.as_image_object())` | pdfium handles filter chains, masks, ICC profiles, color spaces, and all the edge cases. We already depend on it. |
| Image re-encoding to PNG | Writing PNG chunks by hand | `image::DynamicImage::write_to(&mut Cursor, ImageFormat::Png)` | Already used in `pdf_raster.rs:67-71`. |
| Cache sharding / path computation | New hash-to-path logic | Existing `cache_path_for_hash(data_dir, hex)` | Already emits `asset-cache/{shard}/{hex}.txt` with sharding. `[VERIFIED: src/pipeline/assets/cache.rs:13-24]` |
| Markdown blockquote escaping | Complex escape of description content | Prefix each line with `> ` | Markdown blockquotes are line-prefixed; pulldown-cmark (used by the chunker) handles this correctly. |
| SHA-256 of source bytes | Custom hasher | `sha2::Sha256` with `Digest::finalize()` | Already used in `ingest.rs:117-122`. |
| Anthropic vision HTTP call | New HTTP client | `AnthropicAssetClient::describe_image(media_type, bytes)` | Already public, already handles auth, retries, error shapes. `[VERIFIED: src/pipeline/assets/anthropic_extract.rs:69-143]` |
| Double-index prevention | New ChunkStore guard | `prune_absent_markdown_files` (Phase 9 ship) + walker `.md`-only extension filter | Already implemented per CONTEXT D-10. Phase 11 only updates README. |

**Key insight:** Every primitive Phase 11 needs is already in the codebase or in a dependency we already pull in. The work is structural composition — move a hash up, add a per-page loop, wrap strings in `> **[Image: …]** …` — not algorithm invention.

## Runtime State Inventory

Phase 11 is a code extension within an existing pipeline. No renames, no datastore reorganization, no OS-level state registration.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — LanceDB schema is unchanged (no new `source_hash` column per CONTEXT deferred ideas). The `asset-cache/{shard}/{sha256}.txt` layout is preserved; Phase 11 only adds **reads** of these files (writes already exist in Phase 9). | None |
| Live service config | None — no Anthropic dashboard, no registered webhook. Model ID `claude-3-5-haiku-20241022` and `LOCAL_INDEX_ASSET_MODEL` override are untouched. | None |
| OS-registered state | None — no OS-level registrations. | None |
| Secrets/env vars | `ANTHROPIC_API_KEY`, `LOCAL_INDEX_ASSET_MODEL`, `LOCAL_INDEX_ANTHROPIC_BASE_URL`, `LOCAL_INDEX_MAX_PDF_PAGES`, `LOCAL_INDEX_MAX_ASSET_BYTES`, `LOCAL_INDEX_OCR_PROVIDER`, `LOCAL_INDEX_SKIP_ASSET_PROCESSING`. Phase 11 reads the same set — no new env vars required. | None (planner may introduce e.g. `LOCAL_INDEX_EMBEDDED_IMAGE_VISION=true|false` as a kill-switch if desired; not strictly required) |
| Build artifacts | None — no Cargo.toml additions. Existing `pdfium-render 0.8.37` binding loads system pdfium. | None |

**Nothing found in any category requires data migration or manual intervention.** Phase 11 is a pure code rollout.

## Common Pitfalls

### Pitfall 1: pdfium system library not installed on target machine

**What goes wrong:** `Pdfium::bind_to_system_library()` returns `Err` on machines without a `libpdfium` shared library.
**Why it happens:** `pdf_raster.rs` already handles this via a fallback to `pdftoppm`. Phase 11's embedded-image extraction has **no pdftoppm fallback** because pdftoppm does not expose per-image extraction — it only renders pages.
**How to avoid:** (1) If pdfium fails to bind, log WARN and skip embedded-image extraction (TextFirst PDFs still index their text via lopdf — the user doesn't lose all searchability). (2) Document the dependency in README alongside the existing pdfium/poppler install instructions. (3) Consider a feature flag `LOCAL_INDEX_EMBEDDED_IMAGE_VISION=false` kill-switch (Claude's discretion per CONTEXT).
**Warning signs:** User reports "images in my PDFs aren't being described" but CLI output shows successful indexing; check for WARN logs with `pdfium bind` substring.

### Pitfall 2: `get_raw_image()` returns Err for mask/transform-heavy objects

**What goes wrong:** The pdfium example uses `if let Ok(image) = image.get_raw_image()` — i.e., it silently skips images that fail to decode. These can be image masks (alpha channels), soft masks, or objects with transforms that pdfium can't reduce to a single bitmap.
**Why it happens:** PDF images can be layered (base + mask), and "raw" extraction only returns a useful bitmap when the image has a concrete pixel buffer.
**How to avoid:** Follow the example pattern — `if let Ok(dyn_image) = img_obj.get_raw_image() { ... }`. Do NOT propagate the Err; the alternative (rendering the full page and cropping via bounds) is Phase 11-scope overkill and is explicitly in deferred ideas.
**Warning signs:** Count of embedded images described < count of `PdfPageObject::Image` variants. Not catastrophic; log at DEBUG.
`[CITED: https://github.com/ajrcarey/pdfium-render/blob/master/examples/image_extract.rs]`

### Pitfall 3: PDF coordinate origin confusion (bottom-left vs top-left)

**What goes wrong:** If planner opts for positional interleaving (Claude's discretion, D-07), a naive "sort by top() ascending" puts images at the bottom of the page (because PDFium's origin is bottom-left, so a "top" of 800 points is high on the page).
**Why it happens:** PDF coordinate system per `[CITED: https://docs.rs/pdfium-render/0.8.37/pdfium_render/prelude/struct.PdfQuadPoints.html]` has origin (0,0) at page bottom-left.
**How to avoid:** Sort by `bounds.top()` **descending** (highest Y = visual top). **Simpler recommendation:** skip positional interleaving entirely for Phase 11 — use "page text, then image blockquotes in pdfium iteration order" layout. This is trivially correct and requires no coordinate reasoning.
**Warning signs:** Test fixtures with known layouts produce markdown where image descriptions appear in visibly wrong order relative to text.

### Pitfall 4: Corrupt cache causes infinite re-fetch loop

**What goes wrong:** If `read_cache_if_present` returns `None` but the cache write also fails, every re-index triggers a new API call (no idempotency for the broken file).
**Why it happens:** Partial writes, disk-full, permissions issues.
**How to avoid:** (1) Cache writes use `ensure_cache_parent` to create dirs; existing code already handles the error at DEBUG level. (2) Consider upgrading the write-failure log from DEBUG to WARN in Phase 11 so operators see the loop symptom. (3) `tokio::fs::write` is not atomic (not a rename-into-place); a crash mid-write leaves a partial file. Use `tokio::fs::write` to a `.tmp` path then `rename` into place if this becomes a real issue — **defer for now**, Phase 11 does not introduce this bug (Phase 9's write had the same property).
**Warning signs:** `corrupt_cache = true` WARN logs repeating for the same path across runs.

### Pitfall 5: Cache hit masks changes to the Anthropic model or prompt

**What goes wrong:** Operator upgrades to a newer Claude model or changes `ASSET_VISION_PROMPT`, but cache files from the previous model/prompt are still used — re-indexing produces stale descriptions.
**Why it happens:** The cache key is the source SHA-256 only — it does not include the model ID or prompt hash.
**How to avoid:** **(a)** Document: "to invalidate the cache, delete `.local-index/asset-cache/`". **(b)** Planner discretion: include `model_id` and/or `ASSET_VISION_PROMPT` hash in the cache key (e.g., `asset-cache/{model_hash}/{shard}/{sha256}.txt`). Recommendation: **do not add model to the key in Phase 11** — PRE-04 talks about source-hash idempotency, not model-hash idempotency, and the existing LanceDB `content_hash` check protects re-embeds. Document the invalidation procedure.
**Warning signs:** After changing `LOCAL_INDEX_ASSET_MODEL`, re-indexed chunks still reference old descriptions.

### Pitfall 6: Embedded images in encrypted or permission-restricted PDFs

**What goes wrong:** `Pdfium::load_pdf_from_byte_slice(bytes, None)` fails for password-protected PDFs (the second argument is the password).
**Why it happens:** Same behavior already exists in `pdf_raster.rs` for NeedsVision PDFs.
**How to avoid:** Existing error propagation is sufficient; the asset is skipped with a WARN just as before.
**Warning signs:** Specific PDF consistently fails to index.

### Pitfall 7: Blockquote format breaking the smart chunker

**What goes wrong:** The Phase 1 chunker (`src/pipeline/chunker.rs`) splits by heading and by size, with scoring for structural break points. Blockquotes are a structural element in pulldown-cmark.
**Why it happens:** Blockquotes can span many lines; if a `> **[Image: ...]** ...` block ends up larger than CHUNK_SIZE_CHARS (3600 chars), it will be split mid-blockquote.
**How to avoid:** Vision descriptions are typically < 500 chars, so this is unlikely in practice. Observe: the chunker scans for newline-anchored breakpoints and will not truncate mid-word. Test with a representative PDF; no code change needed unless very-long descriptions become a pattern.
**Warning signs:** Chunks containing `> **[Image: ...]**` followed by plain text (wrong — the `>` should prefix every continuation line).

## Code Examples

### Cache read before API call (D-02)

```rust
// Source: synthesized; new helper in src/pipeline/assets/cache.rs per Phase 11 plan
pub async fn read_cache_if_present(path: &Path) -> Option<String> {
    match tokio::fs::read_to_string(path).await {
        Ok(s) if !s.trim().is_empty() => Some(s),
        Ok(_) => {
            tracing::warn!(
                corrupt_cache = true,
                path = %path.display(),
                "asset cache file exists but is empty; refetching from API"
            );
            None
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            tracing::warn!(
                corrupt_cache = true,
                path = %path.display(),
                error = %e,
                "asset cache read failed; refetching from API"
            );
            None
        }
    }
}
```

### Iterate page objects, filter images, re-encode PNG

```rust
// Source: https://github.com/ajrcarey/pdfium-render/blob/master/examples/image_extract.rs
// Adapted for local-index return shape and error handling.
use std::io::Cursor;
use image::ImageFormat;
use pdfium_render::prelude::*;

for (page_index, page) in doc.pages().iter().enumerate() {
    for (object_index, object) in page.objects().iter().enumerate() {
        if let Some(image) = object.as_image_object() {
            if let Ok(dyn_image) = image.get_raw_image() {
                let mut png_bytes = Vec::new();
                if dyn_image
                    .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
                    .is_ok()
                {
                    // png_bytes is safe to send to describe_image("image/png", _)
                }
            }
        }
    }
}
```

### Anthropic vision call (reuse existing)

```rust
// Source: src/pipeline/assets/anthropic_extract.rs (UNCHANGED)
let desc = client.describe_image("image/png", &png_bytes).await?;
// desc is the plaintext description returned by Claude.
```

### Blockquote formatting (D-04, D-05)

```rust
fn blockquote_image(filename: &str, description: &str) -> String {
    let mut out = format!("> **[Image: {filename}]** ");
    let mut first = true;
    for line in description.lines() {
        if first {
            out.push_str(line);
            first = false;
        } else {
            out.push_str("\n> ");
            out.push_str(line);
        }
    }
    out
}
```

### Standalone image output (PRE-12)

```rust
// Source: synthesized; replaces lines 94-108 in ingest.rs
let desc = c.describe_image(mt, &bytes).await?;
let block = blockquote_image(fname, &desc);
let synthetic = format!("# {fname}\n\n{block}\n");
```

### NeedsVision per-page blockquote wrapping (D-04)

```rust
// Source: synthesized; replaces lines 74-92 in ingest.rs
let pngs = rasterize_pdf_pages_to_png(&bytes, max_pdf_pages)?;
if pngs.is_empty() {
    return Err(LocalIndexError::Config(
        "no pages rasterized from PDF".to_string(),
    ));
}
let ocr_parts = ocr.ocr_scanned_pdf_pages(&pngs).await?;
let stem = Path::new(fname).file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
let parts: Vec<String> = ocr_parts
    .into_iter()
    .enumerate()
    .map(|(i, text)| {
        let synthetic_name = format!("{stem}_page_{}.png", i + 1);
        blockquote_image(&synthetic_name, &text)
    })
    .collect();
let body = parts.join("\n\n---\n\n");
let synthetic = format!("# Source: {fname}\n\n{body}");
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-page Anthropic vision for NeedsVision PDFs, flat markdown output | Same call path, but output wrapped in `> **[Image: stem_page_N.png]** {ocr_text}` blockquote | Phase 11 (this phase) | Descriptions are more prominent in chunks; retrieved results clearly identify image-sourced content; no breaking change to chunker. |
| `extract_text_pdf_as_markdown` returns one flat body per TextFirst PDF | Per-page text + per-page embedded-image descriptions, interleaved | Phase 11 (this phase) | TextFirst PDFs with figures (papers, reports) now have those figures indexed; previously those figures were invisible to search. |
| Cache file was write-only (Phase 9 wrote but never read) | Cache file is **read first**; write only on miss | Phase 11 (this phase) | API-cost-bearing operations (vision, OCR) skip on unchanged source. |
| `PdfPageObject::bounds()` returned `PdfRect` | Returns `PdfQuadPoints` (since pdfium-render 0.8.28) | Already in pdfium-render 0.8.37 (current pin) | Planner must use `bounds.top()/bottom()/left()/right()` methods, not `.top` / `.bottom` fields (deprecated). `[CITED: https://docs.rs/pdfium-render/0.8.37/pdfium_render/prelude/struct.PdfQuadPoints.html]` |

**Not deprecated, but noteworthy:**

- Anthropic Files API (upload image once, reference by `file_id` in subsequent calls) exists and would reduce payload size for PDFs with many embedded images. **Defer** — Phase 11 scope is "first make it work"; existing `describe_image` uses base64 inline which is fine for a few images per PDF. `[CITED: https://platform.claude.com/docs/en/build-with-claude/vision]`
- Claude Sonnet 4.6 supports images up to 1568px long edge; Opus 4.7 supports up to 2576px. Our PNGs from `get_raw_image` reflect the source image's pixel dimensions (no resampling). If an embedded image is larger, Anthropic will downscale server-side — no code change needed, but operators on strict token budgets may want to pre-resize. `[CITED: WebSearch Apr 2026, https://pricepertoken.com]`

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | compile | ✓ | edition 2024 per Cargo.toml | — |
| `pdfium-render` Rust crate | embedded image extraction (new) + page rasterization (existing) | ✓ | 0.8.37 | — |
| `libpdfium` system library | pdfium-render runtime | ✓ or ✗ (host-dependent) | macOS: via homebrew or bundled; Linux: distro-specific | For **rasterization only:** `pdftoppm` (Poppler) — already wired in `pdf_raster.rs`. **For embedded image extraction: no fallback** — if pdfium not available, embedded images are silently skipped (text still indexed). Recommend documenting WARN log behavior. |
| `lopdf` Rust crate | PDF classification + page-wise text | ✓ | 0.38 | — |
| `image` Rust crate | DynamicImage → PNG re-encode | ✓ | =0.25.4 with `png` feature | — |
| `sha2` Rust crate | source bytes SHA-256 | ✓ | 0.11 | — |
| `tokio::fs` | async cache read/write | ✓ | 1.40 | — |
| `ANTHROPIC_API_KEY` env var | vision for images (standalone + embedded) and Anthropic OCR | Runtime-provided | — | If missing for standalone images: existing behavior (Credential error). For TextFirst embedded images: **planner's call** — recommend graceful degradation (skip image descriptions, index text). |
| `cargo test` | verification | ✓ | — | — |
| `wiremock` (dev-dep) | Anthropic mock in integration tests | ✓ | 0.6 | — |
| `tempfile` (dev-dep) | tempdir fixtures | ✓ | 3.0 | — |

**Missing dependencies with no fallback:** None that are blocking. pdfium system library is the only soft dependency; Phase 9 already requires it and documents installation.

**Missing dependencies with fallback:** As above — embedded image extraction requires pdfium; if unavailable, degrade to text-only indexing (no hard failure).

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` (tokio::test for async) + `wiremock 0.6` for HTTP mocking |
| Config file | none (Cargo managed) |
| Quick run command | `cargo test --package local-index assets::` (scopes to asset module unit tests) |
| Full suite command | `cargo test --all-targets` (includes `tests/*_integration.rs` + unit tests + wiremock integration) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PRE-04 | Cache hit skips API call | unit (tokio) | `cargo test --lib pipeline::assets::ingest::tests::cache_hit_skips_api -- --nocapture` | Wave 0 |
| PRE-04 | Corrupt/empty cache triggers refetch + WARN | unit (tokio) | `cargo test --lib pipeline::assets::cache::tests::empty_cache_file_returns_none` | Wave 0 |
| PRE-04 | Cache read failure triggers WARN with error field | unit (tokio) | `cargo test --lib pipeline::assets::cache::tests::unreadable_cache_returns_none` | Wave 0 |
| PRE-09 | Standalone image gets blockquote | unit (tokio + wiremock) | `cargo test --lib pipeline::assets::ingest::tests::standalone_image_uses_blockquote` | Wave 0 (replaces/augments existing tests) |
| PRE-09 | Embedded image from TextFirst PDF gets Anthropic-described | integration (wiremock) | `cargo test --test anthropic_assets_mock textfirst_pdf_calls_vision_per_embedded_image` | Wave 0 |
| PRE-10 | TextFirst PDF output interleaves text + image blockquotes per page | unit | `cargo test --lib pipeline::assets::ingest::tests::textfirst_pdf_interleaves_text_and_images` | Wave 0 |
| PRE-10 | Page separator `---` preserved | unit | `cargo test --lib pipeline::assets::ingest::tests::pages_joined_with_separator` | Wave 0 |
| PRE-11 | Blockquote uses `> **[Image: filename]** desc` exact format | unit | `cargo test --lib pipeline::assets::ingest::tests::blockquote_format_matches_spec` | Wave 0 |
| PRE-11 | Filename-only label (not path) | unit | `cargo test --lib pipeline::assets::ingest::tests::blockquote_label_is_filename_only` | Wave 0 |
| PRE-12 | Standalone image markdown has blockquote as body | unit (tokio + wiremock) | (shared with PRE-09 standalone test) | Wave 0 |
| PRE-13 | README describes ephemeral cache approach | manual review / doc test | `cargo doc --no-deps` + manual grep of README | Wave 0 (documentation edit) |

### Sampling Rate

- **Per task commit:** `cargo test --package local-index pipeline::assets::` (~10-20s, unit tests only)
- **Per wave merge:** `cargo test --all-targets` (~30-90s, includes wiremock integration)
- **Phase gate:** `cargo test --all-targets && cargo clippy --all-targets -- -D warnings` green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `src/pipeline/assets/cache.rs` — add unit tests for `read_cache_if_present` (hit, empty, IO error, NotFound)
- [ ] `src/pipeline/assets/ingest.rs` — add unit tests for cache-hit fast-path (no API call when cache file present)
- [ ] `src/pipeline/assets/ingest.rs` — add unit tests for blockquote composition (standalone, NeedsVision, TextFirst embedded)
- [ ] `src/pipeline/assets/pdf_images.rs` (new, if planner extracts module) — add unit tests using a synthetic PDF fixture with an embedded PNG (can be constructed with `lopdf` similar to `fixture_single_page_text_pdf`; or add a binary fixture file under `tests/fixtures/`)
- [ ] `tests/anthropic_assets_mock.rs` — extend with a TextFirst-with-image fixture to assert embedded-image vision is called N times per page
- [ ] README.md — Phase 11 closes PRE-13: explicit section on ephemeral cache + double-index prevention (Phase 9 already implemented the code)

No new test framework install required — all current frameworks (cargo test, wiremock, tempfile) cover Phase 11.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Anthropic API key is the only credential; handled by Phase 9/10 via `resolve_anthropic_key_for_assets`. |
| V3 Session Management | no | No sessions. |
| V4 Access Control | no | Local daemon, 127.0.0.1 default bind. |
| V5 Input Validation | yes | (a) PDF bytes bounded by `max_bytes` check before any parsing (existing). (b) `asset_rel` must canonicalize within `vault` root (existing `starts_with` check in `ingest.rs:41-47`). (c) Vision response text is treated as opaque string and embedded verbatim in markdown — malicious descriptions cannot execute JS because chunks are plain text in LanceDB; the web dashboard displays chunks via `askama` which escapes by default. |
| V6 Cryptography | yes | SHA-256 via `sha2` crate — standard library, no hand-roll. No new crypto in Phase 11. |

### Known Threat Patterns for Rust + pdfium

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malicious PDF triggers native crash in pdfium | Denial of Service | `pdfium-render` wraps pdfium's C API; the wrapper returns `Result`; pdfium itself is hardened (Chromium upstream). **Planner action:** rely on `Result` propagation; do not add panic hooks. Phase 9's `Pdfium::bind_to_system_library().ok()?` pattern already short-circuits on binding failure. Large PDF DoS is mitigated by `max_bytes` gate (existing) and `max_pages` cap (existing, default 30). |
| Malicious vision description contains markdown injection | Tampering | Chunks are indexed as raw text; the search dashboard uses `askama` template escaping. No execution context. **No mitigation needed beyond existing templating.** |
| Cache file path traversal via crafted SHA-256 | Tampering | SHA-256 hex output is always `[0-9a-f]{64}` — no path separators. `cache_path_for_hash` builds the path from fixed shards; no operator input flows into the path. `[VERIFIED: src/pipeline/assets/cache.rs:13-24]` |
| Symlink attack in vault causes asset read outside vault | Elevation | Existing `canonicalize()` + `starts_with(vault)` guard in `ingest_asset_path:39-47` handles this. No Phase 11 change. |
| Cache poisoning via hash collision | Tampering | SHA-256 collisions are cryptographically infeasible. Not a real threat. |
| Image data sent to Anthropic leaks sensitive content from PDFs | Information disclosure | **Policy decision, not a code issue.** Operator opts in by setting `ANTHROPIC_API_KEY` and running asset processing. Document: "Asset processing sends your PDF/image contents to Anthropic for semantic extraction." Already applies pre-Phase 11; no new exposure. |

### Phase 11 Security Checklist

- [ ] Cache path is derived solely from fixed `data_dir` + SHA-256 hex; no user-controlled path segments
- [ ] Source bytes are read with `max_bytes` cap **before** any parsing (existing)
- [ ] Asset path canonicalization + vault prefix check (existing; unchanged)
- [ ] Vision description is treated as opaque string, inserted into markdown via blockquote formatting — no shell or SQL context
- [ ] No new secrets introduced; existing `ANTHROPIC_API_KEY` is the only credential touched
- [ ] Corrupt-cache handling degrades safely (refetch) rather than crashing the asset pipeline

## Project Constraints (from CLAUDE.md)

- **Tech stack:** Rust only — no Node/Python helpers. **Phase 11 compliance:** all new code is Rust; pdfium-render is a Rust crate with a C library binding (existing dependency).
- **Embeddings:** Voyage AI API. **Phase 11 compliance:** no changes to embedder — synthetic markdown flows through the same `chunk_markdown` + Voyage path.
- **CLI framework:** `clap` with derive. **Phase 11 compliance:** no new CLI flags strictly required; if planner adds `--embedded-image-vision` or similar kill-switch, use `clap` derive with `env = "LOCAL_INDEX_..."` per existing `skip-asset-processing` pattern.
- **Logging:** `tracing` crate (no `log`). **Phase 11 compliance:** WARN logs per D-03 use `tracing::warn!(...)` with structured fields matching existing `ingest.rs` style.
- **Metrics:** Prometheus-compatible. **Phase 11 compliance:** optional — planner may add a `cache_hits` / `cache_misses` counter for observability (follows existing `metrics::*` facade pattern).
- **Deployment:** Single binary. **Phase 11 compliance:** no new binaries, no new processes.
- **Diagrams:** All diagrams must use Mermaid. **Phase 11 compliance:** this research's architecture diagram uses Mermaid (see above); any further diagrams in PLAN.md or README must follow.
- **GSD workflow enforcement:** Phase 11 code changes go through `/gsd:execute-phase`. **Research phase only writes this file.**
- **Architecture convention — follow existing patterns:** Asset pipeline uses a flat module under `src/pipeline/assets/`; Phase 11 follows this (new `pdf_images.rs` sibling, or fold into `ingest.rs`).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `PdfPageImageObject::get_raw_image()` returns a `DynamicImage` that re-encodes cleanly to PNG for all real-world PDF embedded images | Architecture Patterns / Pattern 2 | Some image masks or device-specific color spaces may decode with unexpected alpha/color; worst case a subset of embedded images produces visually degraded PNGs. Anthropic vision should still describe them. Verified via pdfium-render example source. Risk: LOW. |
| A2 | Anthropic vision API accepts re-encoded PNG bytes from `DynamicImage::write_to(_, ImageFormat::Png)` at sizes typical of PDF embedded images (<1MP) | Architecture Patterns | If sizes exceed 8000x8000 or 5MB, Anthropic rejects. Embedded PDF images are typically smaller. Risk: LOW. |
| A3 | Planner will choose "text first, then image blockquotes per page" layout rather than positional interleaving | Pattern 4 | If planner chooses positional, the coordinate-system pitfall (Pitfall 3) applies; research documents both paths. Risk: MEDIUM if ignored. |
| A4 | Cache-file-existence is sufficient for PRE-04 idempotency (no model/prompt hash in key) | Pitfall 5, User Constraints | If operators change models or prompts without deleting the cache, they get stale descriptions. Documented as a known behavior. Risk: MEDIUM — documented, not silently incorrect. |
| A5 | Graceful degradation when `ANTHROPIC_API_KEY` missing on TextFirst embedded images is preferred over hard failure | Pattern 4 note | If planner / user prefers hard failure for consistency with standalone images, that's a one-line change; current standalone images error when key missing. Risk: LOW. |
| A6 | pdfium-render 0.8.37's `image` feature is enabled in our current dependency spec (required for `get_raw_image()` to exist) | Architecture Patterns | If feature is disabled, `get_raw_image()` is not available and code won't compile. Need to verify during plan execution by trying a minimal `cargo check` with the new code. Risk: LOW (pdfium-render has `image` in default features; Cargo.toml doesn't use `default-features = false` for pdfium-render). Planner should confirm in Wave 0. |
| A7 | The existing ephemeral cache layout in `.local-index/asset-cache/{shard}/{sha256}.txt` is stable and will not collide with future cache uses | cache.rs | Existing layout has no versioning; if a future phase needs per-model caches, it would need to migrate. Risk: LOW for Phase 11 scope. |

## Open Questions

1. **Should Phase 11 add a `--no-embedded-image-vision` kill switch?**
   - What we know: CONTEXT D-09 says all pages with ≥1 embedded image get vision called; no filtering. D-07 calls for the feature unconditionally.
   - What's unclear: Operators with large PDFs and many figures may want to disable this to control cost without disabling all asset processing.
   - Recommendation: **Optional for Phase 11.** Default on. If added, use `--skip-embedded-image-vision` CLI flag + `LOCAL_INDEX_SKIP_EMBEDDED_IMAGE_VISION` env var, matching `--skip-asset-processing` pattern.

2. **Should the cache key include model ID and/or prompt hash?**
   - What we know: A4 above. PRE-04 wording talks about source content hash only.
   - What's unclear: User expectation when switching models.
   - Recommendation: **Defer.** Document cache invalidation procedure in README (delete `.local-index/asset-cache/` or a specific shard).

3. **For TextFirst PDFs with embedded images but `ANTHROPIC_API_KEY` not set — error or degrade?**
   - What we know: Standalone images today error (`LocalIndexError::Credential`).
   - What's unclear: Whether PDFs-with-figures should also error, or should still index text with missing image descriptions.
   - Recommendation: **Degrade gracefully with WARN.** This aligns with the "Anthropic is optional for TextFirst classification path" intent. Planner decides finally.

4. **Page-level position reassembly vs simple "text then images per page" layout?**
   - What we know: CONTEXT D-07 leaves this as Claude's discretion. Positional interleaving requires reconciling lopdf text positions with pdfium image positions.
   - What's unclear: Whether PDE-10 success criterion 2 ("interleaves text and image descriptions") demands true spatial interleaving or is satisfied by per-page interleaving.
   - Recommendation: **Per-page interleaving only for Phase 11.** Text first, then all image blockquotes. Simple and testable. Deferred: true positional interleaving is a follow-up if quality warrants.

5. **Synthetic filename format for embedded images — `{stem}_page_{n}_image_{i}.png` vs other?**
   - What we know: pdfium example uses `image-test-page-0-image-2.jpg`. SEED-001 used `figure_1.png`.
   - What's unclear: Convention.
   - Recommendation: **`{stem}_page_{n}_image_{i}.png`** — 1-based page and image indices, stable, informative. Update README with the convention.

## Sources

### Primary (HIGH confidence)

- [pdfium-render 0.8.37 docs.rs](https://docs.rs/pdfium-render/0.8.37/pdfium_render/) — API surface of `PdfPageObject`, `PdfPageImageObject`, `PdfQuadPoints`, `PdfPageObjectCommon`
- [pdfium-render PdfPageImageObject docs.rs](https://docs.rs/pdfium-render/0.8.37/pdfium_render/prelude/struct.PdfPageImageObject.html) — `get_raw_image()` vs `get_raw_image_data()`, `width()`, `height()`
- [pdfium-render PdfQuadPoints docs.rs](https://docs.rs/pdfium-render/0.8.37/pdfium_render/prelude/struct.PdfQuadPoints.html) — coordinate system, `top()`/`bottom()`/`left()`/`right()`, PDF origin bottom-left
- [pdfium-render PdfPageObject docs.rs](https://docs.rs/pdfium-render/0.8.37/pdfium_render/prelude/enum.PdfPageObject.html) — `as_image_object()`, variants, `object_type()`
- [pdfium-render image_extract.rs example](https://github.com/ajrcarey/pdfium-render/blob/master/examples/image_extract.rs) — canonical iteration + extraction pattern
- [pdfium-render examples README](https://github.com/ajrcarey/pdfium-render/blob/master/examples/README.md) — index of examples
- [Anthropic Vision guide](https://platform.claude.com/docs/en/build-with-claude/vision) — base64 image content blocks, media types, size limits
- Codebase: `src/pipeline/assets/ingest.rs`, `pdf_local.rs`, `pdf_raster.rs`, `anthropic_extract.rs`, `cache.rs`, `ocr.rs`, `chunker.rs` — existing integration points and patterns
- `Cargo.toml` and `Cargo.lock` — exact dependency versions (pdfium-render 0.8.37, image =0.25.4, lopdf 0.38, sha2 0.11)

### Secondary (MEDIUM confidence)

- [pdfium-render crates.io](https://crates.io/crates/pdfium-render) — crate metadata, release notes summary
- [pdfium-render 0.8.37 changelog mention (WebSearch)](https://docs.rs/crate/pdfium-render/latest) — `get_raw_image_data()` added in 0.8.37
- Anthropic model pricing and image support 2026 (WebSearch) — model IDs, image size limits for Sonnet (1568px) vs Opus (2576px)

### Tertiary (LOW confidence)

- PDF filter conventions (DCTDecode, JPXDecode, FlateDecode, CCITTFaxDecode) (WebSearch) — used only to justify the choice of `get_raw_image()` over `get_raw_image_data()`. Low confidence does not affect the recommendation: decoded `DynamicImage` is unambiguous regardless.

## Metadata

**Confidence breakdown:**

- Standard stack: **HIGH** — all deps already pinned in Cargo.toml; pdfium-render 0.8.37 documented API surface verified on docs.rs.
- Architecture: **HIGH** — existing pipeline structure is well-understood; Phase 11 is pure extension of `ingest.rs` orchestration + one new small module.
- Cache idempotency pattern (D-02): **HIGH** — existing `cache_path_for_hash` helper; just add a read helper.
- Blockquote format: **HIGH** — exact format spec in CONTEXT D-04/D-05 and SEED-001.
- pdfium API (`get_raw_image`, `as_image_object`, `PdfQuadPoints`): **HIGH** — confirmed via docs.rs and official example.
- Page-order interleaving strategy: **MEDIUM** — two viable strategies (per-page vs positional); recommendation leans per-page (simplest). Planner chooses.
- Pitfalls: **HIGH** — sourced from pdfium docs, prior phase code, and coordinate-system mechanics.
- Fallback when pdfium unavailable on target machine: **MEDIUM** — behavior should be defined in plan (skip+WARN vs fail).

**Research date:** 2026-04-20
**Valid until:** 2026-06-20 (60 days — pdfium-render, Anthropic vision API, and project code are all stable on short timescales; re-verify pdfium-render version if planner delays > 60 days).

## RESEARCH COMPLETE
