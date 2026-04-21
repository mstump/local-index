# Phase 11: Vision enrichment & idempotency - Pattern Map

**Mapped:** 2026-04-20
**Files analyzed:** 7 (4 modified, 1 new, 1 test extension, 1 doc)
**Analogs found:** 7 / 7

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/pipeline/assets/ingest.rs` (MODIFY) | orchestrator | request-response + file-I/O | Self (existing function, same file) — refactor | exact |
| `src/pipeline/assets/cache.rs` (MODIFY) | utility | file-I/O | Self (add `read_cache_if_present` next to `cache_path_for_hash`) + `pdf_raster.rs::try_pdftoppm` for tokio::fs read pattern | exact |
| `src/pipeline/assets/pdf_images.rs` (NEW, optional) | utility | transform (bytes→bytes) | `src/pipeline/assets/pdf_raster.rs::try_pdfium` | exact (same pdfium-render API surface) |
| `src/pipeline/assets/pdf_local.rs` (MODIFY) | utility | transform | Self — add a per-page text extractor `extract_page_text_vec()` alongside `extract_text_pdf_as_markdown` | exact |
| `src/pipeline/assets/ingest.rs::blockquote_image` helper (NEW, private fn) | formatter | transform | Small formatter — no direct analog; follow string-composition style already in `ingest.rs:72,91,108` | role-match |
| `tests/anthropic_assets_mock.rs` (EXTEND) | test (integration) | request-response | Self + `ingest.rs` test `needs_vision_pdf_routes_raster_pages_through_ocr_service` (lines 168-215) | exact |
| `README.md` (MODIFY, PRE-13 doc) | config/docs | n/a | Existing README section around line 105 (asset-cache paragraph) | role-match |

## Pattern Assignments

### `src/pipeline/assets/cache.rs` (utility, file-I/O) — ADD `read_cache_if_present`

**Analog:** Self (extends existing module) + `pdf_raster.rs::try_pdftoppm` (lines 79-117) for `tokio::fs` error-kind handling

**Imports pattern** (cache.rs lines 1-3 — extend with `tokio::fs` use inline or at top):
```rust
//! Ephemeral on-disk cache paths under the configured data directory.

use std::path::{Path, PathBuf};
```

**Existing helper style to mirror** (cache.rs lines 13-24):
```rust
pub fn cache_path_for_hash(data_dir: &Path, sha256_hex: &str) -> PathBuf {
    let shard = if sha256_hex.len() >= 4 {
        format!(
            "{}/{}",
            &sha256_hex[..2],
            &sha256_hex[2..4]
        )
    } else {
        "_/_".to_string()
    };
    cache_dir(data_dir).join(shard).join(format!("{sha256_hex}.txt"))
}
```

**Error-kind handling pattern** (WARN + fallback shape from existing code — e.g., `ingest.rs:124-127`):
```rust
if let Err(e) = ensure_cache_parent(&cache_path) {
    tracing::debug!(error = %e, "asset cache mkdir skipped");
} else if let Err(e) = tokio::fs::write(&cache_path, synthetic.as_bytes()).await {
    tracing::debug!(error = %e, path = %cache_path.display(), "asset cache write skipped");
}
```

**Concrete new helper** (per RESEARCH.md Pattern 1) — treat `ErrorKind::NotFound` as normal miss (silent), warn on any other IO error or empty-trim content:
```rust
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

**Existing unit-test style to mirror** (cache.rs lines 34-50):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cache_path_for_hash_is_stable() {
        let dir = tempdir().unwrap();
        let h = "deadbeef".repeat(8);
        let p1 = cache_path_for_hash(dir.path(), &h);
        let p2 = cache_path_for_hash(dir.path(), &h);
        assert_eq!(p1, p2);
        ...
    }
}
```
Apply: add `#[tokio::test]` cases for `read_cache_if_present` covering hit, empty-file, NotFound, and non-NotFound IO error paths.

---

### `src/pipeline/assets/pdf_images.rs` (utility, transform) — NEW

**Analog:** `src/pipeline/assets/pdf_raster.rs::try_pdfium` (lines 52-74) — same pdfium binding + document load pattern, same `DynamicImage → PNG` re-encode pattern

**Imports pattern** (pdf_raster.rs lines 11-17):
```rust
use std::io::Cursor;
use std::path::Path;
use std::process::Command;

use image::ImageFormat;
use pdfium_render::prelude::*;

use crate::error::LocalIndexError;
```
For `pdf_images.rs`: drop the `std::path::Path` and `std::process::Command` imports (no subprocess fallback); keep `Cursor`, `ImageFormat`, `pdfium_render::prelude::*`, and `LocalIndexError`.

**Core transform pattern to copy** (pdf_raster.rs lines 52-74):
```rust
fn try_pdfium(pdf_bytes: &[u8], max_pages: usize) -> Option<Vec<Vec<u8>>> {
    let bindings = Pdfium::bind_to_system_library().ok()?;
    let pdfium = Pdfium::new(bindings);
    let doc = pdfium.load_pdf_from_byte_slice(pdf_bytes, None).ok()?;

    let render_config = PdfRenderConfig::new()
        .set_target_width(1024)
        .set_maximum_height(1024);

    let mut out = Vec::new();
    for (idx, page) in doc.pages().iter().enumerate() {
        if idx >= max_pages {
            break;
        }
        let image = page.render_with_config(&render_config).ok()?.as_image();
        let mut buf = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .ok()?;
        out.push(buf);
    }
    Some(out)
}
```

**Apply to `pdf_images.rs`:** keep the `Pdfium::bind_to_system_library().ok()?` gate and the `doc.pages().iter().enumerate()` loop with `max_pages` break; swap `page.render_with_config(...)` for `page.objects().iter()` + `object.as_image_object()` + `img_obj.get_raw_image()` (per RESEARCH.md Pattern 2). The PNG re-encode block is identical (lines 67-71).

**Graceful-degrade pattern** (pdf_raster.rs returns `Option<Vec<Vec<u8>>>` from `try_pdfium` so callers can fall through to fallback): mirror this — `extract_embedded_images_per_page` should return `Result<Vec<Vec<Vec<u8>>>, LocalIndexError>` where the pdfium bind failure maps to a specific error variant OR (preferred — per research Pitfall 1) returns `Ok(vec![])` + emits a WARN, so a missing pdfium does not fail the whole asset.

**Error wrapping pattern** (LocalIndexError::Config with short context; from `pdf_raster.rs:45-49` and `pdf_local.rs:47`):
```rust
.map_err(|e| LocalIndexError::Config(format!("pdfium load: {e}")))
```

**Test fixture pattern** (pdf_local.rs lines 88-147): `fixture_single_page_text_pdf` synthesizes a PDF with `lopdf` for unit tests. For `pdf_images.rs` unit tests, either (a) extend with a new `fixture_single_page_pdf_with_embedded_image()` helper that adds an `/XObject /Subtype /Image` stream via `lopdf::dictionary!` + small PNG bytes, or (b) rely on an integration test with a binary fixture under `tests/fixtures/` (see test section below).

---

### `src/pipeline/assets/pdf_local.rs` (utility, transform) — ADD per-page text extractor

**Analog:** Self — `extract_text_pdf_as_markdown` (lines 64-85) is the existing per-PDF text extractor; Phase 11 needs a sibling `extract_page_text_vec` that returns `Vec<String>` (one entry per page) instead of a flat joined markdown string.

**Core pattern to mirror** (lines 64-85):
```rust
pub fn extract_text_pdf_as_markdown(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<String, LocalIndexError> {
    ensure_under_cap(bytes, max_bytes)?;
    let doc = Document::load_mem(bytes).map_err(LocalIndexError::Pdf)?;
    let page_numbers: Vec<u32> = doc.get_pages().keys().cloned().collect();
    let mut parts = Vec::new();
    for pn in page_numbers {
        let page_text = doc.extract_text(&[pn]).map_err(LocalIndexError::Pdf)?;
        let trimmed = page_text.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }
    let body = parts.join("\n\n");
    if body.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("# Extracted PDF text\n\n{body}"))
    }
}
```

**Apply:** a new `pub fn extract_page_text_vec(bytes: &[u8], max_bytes: usize, max_pages: usize) -> Result<Vec<String>, LocalIndexError>` that preserves empty pages as `String::new()` (so index alignment with `per_page_images` from `pdf_images.rs` is stable), and caps iteration at `max_pages` to match the rasterization cap.

**Error-propagation style to copy** (pdf_local.rs:47, 52, 69, 73):
```rust
Document::load_mem(bytes).map_err(LocalIndexError::Pdf)?;
// ...
doc.extract_text(&[pn]).map_err(LocalIndexError::Pdf)?;
```

---

### `src/pipeline/assets/ingest.rs` (orchestrator, request-response + file-I/O) — MODIFY heavily

**Analog:** Self — the Phase 11 changes are a structural refactor of `ingest_asset_path` (current lines 29-131).

**Imports pattern to extend** (ingest.rs lines 1-14) — add `read_cache_if_present` and (if new module) `pdf_images` imports:
```rust
use std::path::Path;

use sha2::{Digest, Sha256};

use super::anthropic_extract::AnthropicAssetClient;
use super::cache::{cache_path_for_hash, ensure_cache_parent};
use super::ocr::OcrService;
use super::pdf_local::{classify_pdf, extract_text_pdf_as_markdown, PdfClassification};
use super::pdf_raster::rasterize_pdf_pages_to_png;
use crate::error::LocalIndexError;
use crate::pipeline::chunker::chunk_markdown;
use crate::types::ChunkedFile;
```
Add: `use super::cache::read_cache_if_present;` (new helper), `use super::pdf_images::extract_embedded_images_per_page;` (if new module), `use super::pdf_local::extract_page_text_vec;`.

**Existing SHA-256 computation to MOVE UP** (ingest.rs lines 115-123):
```rust
// Optional cache write (debug / retry aid) — does not affect chunk provenance.
let mut hasher = Sha256::new();
hasher.update(&bytes);
let hex: String = hasher
    .finalize()
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect();
let cache_path = cache_path_for_hash(data_dir, &hex);
```
Move this block to **immediately after the size check** (after line 55, before line 57 `let fname = ...`). Then insert a `read_cache_if_present(&cache_path).await` gate that returns early via `chunk_markdown(&cached, asset_rel)` on hit.

**Existing synthetic-markdown composition patterns to wrap in blockquote helper:**

Standalone image (lines 94-108):
```rust
} else if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
    let Some(mt) = media_type_for_image(asset_rel) else {
        return Err(LocalIndexError::Config(format!(
            "unsupported image extension: {ext}"
        )));
    };
    let Some(c) = image_vision else {
        return Err(LocalIndexError::Credential(
            "ANTHROPIC_API_KEY is required for image assets. \
             Set ANTHROPIC_API_KEY from https://console.anthropic.com/"
                .to_string(),
        ));
    };
    let desc = c.describe_image(mt, &bytes).await?;
    format!("# {fname}\n\n{desc}")
}
```
**Change per D-04/D-05:** replace `format!("# {fname}\n\n{desc}")` with a blockquote-wrapped body: `format!("# {fname}\n\n{}\n", blockquote_image(fname, &desc))`.

NeedsVision PDF (lines 74-92):
```rust
PdfClassification::NeedsVision => {
    let Some(ocr) = pdf_ocr else {
        return Err(LocalIndexError::Credential(
            "No OCR provider configured for scanned PDFs (NeedsVision). ..."
                .to_string(),
        ));
    };
    let pngs = rasterize_pdf_pages_to_png(&bytes, max_pdf_pages)?;
    if pngs.is_empty() {
        return Err(LocalIndexError::Config(
            "no pages rasterized from PDF".to_string(),
        ));
    }
    let parts = ocr.ocr_scanned_pdf_pages(&pngs).await?;
    let body = parts.join("\n\n---\n\n");
    format!("# Source: {fname}\n\n{body}")
}
```
**Change per D-04:** wrap each `parts[i]` in `blockquote_image(&synthetic_name, &parts[i])` where `synthetic_name = format!("{stem}_page_{}.png", i + 1)`; keep the `---` separator (D-06).

TextFirst PDF (lines 70-73):
```rust
PdfClassification::TextFirst => {
    let body = extract_text_pdf_as_markdown(&bytes, max_bytes)?;
    format!("# Source: {fname}\n\n{body}")
}
```
**Change per D-07/D-08/D-09:** replace the single-call text extraction with a per-page loop that also iterates `extract_embedded_images_per_page(&bytes, max_pdf_pages)?`; for each page, compose `## Page N\n\n{text}\n\n{blockquote_image(...)}` and join pages with `\n\n---\n\n`. See RESEARCH.md Pattern 4 (lines 362-417) for the complete composition.

**Existing cache-write pattern to KEEP** (lines 124-128) — the write still runs on miss, but the read gate eliminates redundant writes on hit:
```rust
if let Err(e) = ensure_cache_parent(&cache_path) {
    tracing::debug!(error = %e, "asset cache mkdir skipped");
} else if let Err(e) = tokio::fs::write(&cache_path, synthetic.as_bytes()).await {
    tracing::debug!(error = %e, path = %cache_path.display(), "asset cache write skipped");
}
```

**New helper — `blockquote_image`** (private fn in ingest.rs, above `ingest_asset_path`):
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
Place next to `media_type_for_image` (currently ingest.rs:16-24) — same "small private helper above the public entrypoint" positioning.

---

### `tests/anthropic_assets_mock.rs` (test, request-response) — EXTEND

**Analog:** Existing file (lines 1-58) for wiremock/Anthropic setup; `ingest.rs::tests::needs_vision_pdf_routes_raster_pages_through_ocr_service` (ingest.rs:168-215) for the "synth PDF + mock server + ingest" integration pattern.

**Wiremock setup pattern to copy** (ingest.rs:170-183 and anthropic_assets_mock.rs:19-33):
```rust
let server = MockServer::start().await;
Mock::given(method("POST"))
    .and(path("/v1/messages"))
    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "content": [{"type": "text", "text": "OCR_PAGE_BODY"}],
        "id": "msg_1",
        "model": "claude",
        "role": "assistant",
        "stop_reason": "end_turn",
        "type": "message",
        "usage": {"input_tokens": 1, "output_tokens": 1}
    })))
    .mount(&server)
    .await;
```

**Client construction pattern** (anthropic_assets_mock.rs:35):
```rust
let client = AnthropicAssetClient::new_for_test("test-api-key", server.uri());
```

**Ingest-through-pipeline pattern** (ingest.rs:196-215):
```rust
let cf = ingest_asset_path(
    vault.path(),
    rel,
    &data_dir,
    pdf_bytes.len() * 2,
    30,
    pdf_ocr.as_ref(),
    Some(&client),
)
.await
.expect("ingest needs-vision pdf");

assert!(
    cf.chunks
        .iter()
        .any(|c| c.body.contains("OCR_PAGE_BODY")),
    ...
);
```

**Received-request assertion pattern** (anthropic_assets_mock.rs:44-56) — useful for asserting vision was called N times for N embedded images:
```rust
let reqs = server.received_requests().await.unwrap();
let last = reqs.last().expect("one request");
let body: serde_json::Value = serde_json::from_slice(&last.body).unwrap();
// inspect content blocks, assert image source / base64, etc.
```

**Apply to new test `textfirst_pdf_calls_vision_per_embedded_image`:** (1) build a PDF with an embedded image using `lopdf::dictionary!` + PNG stream (mirror `fixture_single_page_text_pdf` in pdf_local.rs:89-147 but with an `/XObject /Subtype /Image` resource referenced by `/Do` in the content stream); (2) configure wiremock; (3) call `ingest_asset_path`; (4) assert `server.received_requests().await.unwrap().len() == 1` (one embedded image = one vision call) and that the chunk body contains `> **[Image: {stem}_page_1_image_1.png]**`.

---

### `README.md` (docs, PRE-13)

**Analog:** Existing asset-cache paragraph at README.md:105 and surrounding Phase 9 description.

**Pattern:** append a subsection describing (1) ephemeral cache layout (`.local-index/asset-cache/{shard}/{sha256}.txt`), (2) double-index prevention (walker excludes raw `.pdf`/`.png`/`.jpg`/`.webp`; `prune_absent_markdown_files` handles deletions), (3) cache invalidation procedure (`rm -rf .local-index/asset-cache/`), (4) no companion files in vault (Phase 9 D-01).

Diagram requirement: if any diagram is included, **MUST be Mermaid** per CLAUDE.md convention. See `11-RESEARCH.md` lines 122-160 for a reusable Mermaid flowchart.

---

## Shared Patterns

### Tracing WARN with structured fields
**Source:** `src/pipeline/assets/ingest.rs:124-127`, `src/main.rs:350,373,379`, `src/daemon/processor.rs:363,386,396,421,442,451,474,489`
**Apply to:** `cache.rs::read_cache_if_present` (D-03), `pdf_images.rs` (pdfium-bind-failure path), `ingest.rs` (WARN when TextFirst PDF has embedded images but `image_vision` is `None`)

Canonical shape (single field):
```rust
tracing::warn!(error = %e, "failed to embed asset");
```

Multi-field shape (file-scoped):
```rust
tracing::warn!(file = %relative_str, error = %e, "failed to ingest asset");
```

Multi-field shape with boolean flag (suits D-03):
```rust
tracing::warn!(
    corrupt_cache = true,
    path = %path.display(),
    "asset cache file exists but is empty; refetching from API"
);
```

### Error wrapping via `LocalIndexError`
**Source:** `src/pipeline/assets/pdf_raster.rs:28, 45-48, 82`; `src/pipeline/assets/pdf_local.rs:23-27, 47`
**Apply to:** All new code paths.

Variants in use:
- `LocalIndexError::Config(String)` — configuration / input-shape errors (rasterize failure, bad path).
- `LocalIndexError::Credential(String)` — missing API key (ingest.rs:76-82, 101-106). Planner must decide per research Open Question #3 whether TextFirst-with-embedded-images-but-no-vision returns `Credential` or warns and degrades; recommendation is graceful degradation.
- `LocalIndexError::AssetTooLarge { bytes, max_bytes }` — size cap (ingest.rs:51-54, pdf_local.rs:22-27).
- `LocalIndexError::Pdf(_)` — wraps `lopdf::Error` (pdf_local.rs:47).
- `LocalIndexError::AssetVision(String)` — wraps Anthropic/DocumentAI HTTP errors (anthropic_extract.rs:110,116,124,137; document_ai.rs).
- `LocalIndexError::Io(std::io::Error)` — file-system I/O.

Mapping style:
```rust
Document::load_mem(bytes).map_err(LocalIndexError::Pdf)?
tokio::fs::read(&abs).await.map_err(LocalIndexError::Io)?
.map_err(|e| LocalIndexError::Config(format!("pdfium load: {e}")))
```

### `max_bytes` size gate BEFORE any parsing
**Source:** `src/pipeline/assets/pdf_local.rs:21-29` (`ensure_under_cap`) and `ingest.rs:50-55`
**Apply to:** Phase 11 already preserves this — do not allow pdfium image extraction to run before the size check. The size check is already performed immediately after `tokio::fs::read` at ingest.rs:50-55; subsequent SHA-256 computation and cache read operate on bounded `bytes`.

### pdfium bind → load pattern
**Source:** `src/pipeline/assets/pdf_raster.rs:53-55`
**Apply to:** `pdf_images.rs::extract_embedded_images_per_page`
```rust
let bindings = Pdfium::bind_to_system_library().ok()?;
let pdfium = Pdfium::new(bindings);
let doc = pdfium.load_pdf_from_byte_slice(pdf_bytes, None).ok()?;
```
Phase 11 does not load pdfium twice in the same branch — TextFirst PDFs go through classify (lopdf) → extract text (lopdf) → extract images (pdfium). Only one pdfium load per ingest call.

### `DynamicImage → PNG` re-encode
**Source:** `src/pipeline/assets/pdf_raster.rs:67-71`
**Apply to:** `pdf_images.rs` (after `get_raw_image()` returns `DynamicImage`) — identical block:
```rust
let mut buf = Vec::new();
dyn_image
    .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
    .ok()?; // or .map_err(...) for Result-flavor
```

### Unit-test scaffold: `#[tokio::test]` + `tempdir` + `wiremock`
**Source:** `src/pipeline/assets/ingest.rs:141-215`
**Apply to:** New tests in `cache.rs` (for `read_cache_if_present`) and `ingest.rs` (for cache-hit fast path, blockquote format, TextFirst embedded-image interleaving). Use `tempdir` to create the data_dir and write a pre-seeded `.local-index/asset-cache/{shard}/{sha256}.txt` to test the fast path without any mock server.

### Module re-export convention
**Source:** `src/pipeline/assets/mod.rs:1-19`
**Apply to:** If `pdf_images.rs` is added as a new module, declare `mod pdf_images;` in `mod.rs` (line 6-13 block) — keep internal; no `pub use` needed unless integration tests outside the crate require it. `extract_embedded_images_per_page` is called only from `ingest.rs` inside the same module tree.

### Optional helper fixture via `#[cfg(test)]`
**Source:** `src/pipeline/assets/pdf_local.rs:88-147` (`fixture_single_page_text_pdf`)
**Apply to:** If Phase 11 adds a PDF fixture with an embedded image, place it as a `#[cfg(test)] pub(crate) fn fixture_single_page_pdf_with_embedded_image() -> Vec<u8>` in `pdf_local.rs` (or `pdf_images.rs`) so tests in `ingest.rs` and integration tests can import via `crate::pipeline::assets::pdf_local::...`.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | — | — | All Phase 11 new/modified files have clear analogs in the existing `src/pipeline/assets/*` modules; no green-field patterns are needed. |

## Metadata

**Analog search scope:**
- `src/pipeline/assets/` (primary insertion surface)
- `src/main.rs`, `src/daemon/processor.rs`, `src/daemon/watcher.rs`, `src/pipeline/walker.rs` (cross-cutting tracing patterns)
- `tests/anthropic_assets_mock.rs` (integration test scaffolding)
- `README.md` (documentation style)

**Files scanned:** `ingest.rs`, `cache.rs`, `pdf_local.rs`, `pdf_raster.rs`, `anthropic_extract.rs`, `ocr.rs`, `document_ai.rs`, `mod.rs`, `anthropic_assets_mock.rs`, `README.md`, plus grep over `tracing::warn!` sites for convention confirmation.

**Pattern extraction date:** 2026-04-20
