//! Orchestrate asset → synthetic markdown → [`crate::pipeline::chunker::chunk_markdown`] (`PRE-01` wiring).

use std::path::Path;

use sha2::{Digest, Sha256};

use super::anthropic_extract::AnthropicAssetClient;
use super::cache::{cache_path_for_hash, ensure_cache_parent, read_cache_if_present};
use super::ocr::OcrService;
use super::pdf_images::extract_embedded_images_per_page;
use super::pdf_local::{classify_pdf, extract_page_text_vec, PdfClassification};
use super::pdf_raster::rasterize_pdf_pages_to_png;
use crate::error::LocalIndexError;
use crate::pipeline::chunker::chunk_markdown;
use crate::types::ChunkedFile;

fn media_type_for_image(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => return None,
    })
}

/// Format a vision description as the canonical image blockquote (`D-04`, `D-05`).
///
/// Label is the filename only (no path components). Multi-line descriptions
/// have every continuation line prefixed with `> ` so the blockquote is
/// one contiguous markdown block.
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

/// Ingest a single vault-relative asset path: classify, extract locally and/or call vision, then chunk.
///
/// `Chunk.file_path` in the returned [`ChunkedFile`] is always `asset_rel` (the original asset).
pub async fn ingest_asset_path(
    vault: &Path,
    asset_rel: &Path,
    data_dir: &Path,
    max_bytes: usize,
    max_pdf_pages: usize,
    pdf_ocr: Option<&OcrService>,
    image_vision: Option<&AnthropicAssetClient>,
) -> Result<ChunkedFile, LocalIndexError> {
    let abs = vault.join(asset_rel);
    let abs = abs.canonicalize().map_err(LocalIndexError::Io)?;
    let vault = vault.canonicalize().map_err(LocalIndexError::Io)?;
    if !abs.starts_with(&vault) {
        return Err(LocalIndexError::Config(format!(
            "asset path {} is outside vault {}",
            abs.display(),
            vault.display()
        )));
    }

    let bytes = tokio::fs::read(&abs).await.map_err(LocalIndexError::Io)?;
    if bytes.len() > max_bytes {
        return Err(LocalIndexError::AssetTooLarge {
            bytes: bytes.len(),
            max_bytes,
        });
    }

    // === PHASE 11: cache-read gate (PRE-04, D-02, D-03) ===
    // SHA-256 over source bytes is computed once, up front; the cache path is
    // derived from that hash. If the cache hits with non-empty contents, every
    // downstream API call (OCR, vision) is skipped.
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let cache_path = cache_path_for_hash(data_dir, &hex);

    if let Some(cached) = read_cache_if_present(&cache_path).await {
        tracing::debug!(
            path = %cache_path.display(),
            "asset cache hit; skipping API"
        );
        return chunk_markdown(&cached, asset_rel);
    }

    let fname = asset_rel
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("asset");

    let ext = asset_rel
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    let synthetic = if ext == "pdf" {
        match classify_pdf(&bytes, max_bytes)? {
            PdfClassification::TextFirst => {
                // Phase 11-02: per-page text (lopdf) aligned with per-page
                // embedded images (pdfium). Each page becomes
                //   ## Page N
                //   {page text}
                //   > **[Image: {stem}_page_N_image_I.png]** {vision desc}
                //   ...
                // and pages are joined with `\n\n---\n\n`.
                let per_page_text = extract_page_text_vec(&bytes, max_bytes, max_pdf_pages)?;
                let per_page_images =
                    extract_embedded_images_per_page(&bytes, max_pdf_pages)?;

                // Graceful degradation per research Open Question #3:
                // if any page has >=1 embedded image but no vision client is
                // configured, log a WARN once and proceed with text-only
                // output for that PDF.
                let any_embedded_images =
                    per_page_images.iter().any(|page_imgs| !page_imgs.is_empty());
                if any_embedded_images && image_vision.is_none() {
                    tracing::warn!(
                        asset = %fname,
                        "TextFirst PDF has embedded images but ANTHROPIC_API_KEY \
                         is not configured; indexing page text only"
                    );
                }

                let stem = Path::new(fname)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("doc");

                let page_count = per_page_text.len().max(per_page_images.len());
                let mut page_blocks: Vec<String> = Vec::with_capacity(page_count);
                for page_idx in 0..page_count {
                    let mut page = format!("## Page {}\n\n", page_idx + 1);
                    if let Some(text) = per_page_text.get(page_idx) {
                        if !text.is_empty() {
                            page.push_str(text);
                            page.push_str("\n\n");
                        }
                    }
                    if let (Some(images), Some(client)) =
                        (per_page_images.get(page_idx), image_vision)
                    {
                        for (img_idx, png) in images.iter().enumerate() {
                            let synthetic_name = format!(
                                "{stem}_page_{}_image_{}.png",
                                page_idx + 1,
                                img_idx + 1,
                            );
                            match client.describe_image("image/png", png).await {
                                Ok(desc) => {
                                    page.push_str(&blockquote_image(&synthetic_name, &desc));
                                    page.push_str("\n\n");
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        asset = %fname,
                                        image = %synthetic_name,
                                        error = %e,
                                        "failed to describe embedded PDF image; \
                                         omitting its blockquote (text still indexed)"
                                    );
                                }
                            }
                        }
                    }
                    page_blocks.push(page.trim_end().to_string());
                }

                let body = page_blocks.join("\n\n---\n\n");
                format!("# Source: {fname}\n\n{body}")
            }
            PdfClassification::NeedsVision => {
                let Some(ocr) = pdf_ocr else {
                    return Err(LocalIndexError::Credential(
                        "No OCR provider configured for scanned PDFs (NeedsVision). \
                         Set ANTHROPIC_API_KEY for the default Anthropic OCR path \
                         (https://console.anthropic.com/)."
                            .to_string(),
                    ));
                };
                let pngs = rasterize_pdf_pages_to_png(&bytes, max_pdf_pages)?;
                if pngs.is_empty() {
                    return Err(LocalIndexError::Config(
                        "no pages rasterized from PDF".to_string(),
                    ));
                }
                let ocr_parts = ocr.ocr_scanned_pdf_pages(&pngs).await?;
                let stem = Path::new(fname)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("doc");
                let parts: Vec<String> = ocr_parts
                    .into_iter()
                    .enumerate()
                    .map(|(i, text)| {
                        let synthetic_name = format!("{stem}_page_{}.png", i + 1);
                        blockquote_image(&synthetic_name, &text)
                    })
                    .collect();
                let body = parts.join("\n\n---\n\n");
                format!("# Source: {fname}\n\n{body}")
            }
        }
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
        let block = blockquote_image(fname, &desc);
        format!("# {fname}\n\n{block}\n")
    } else {
        return Err(LocalIndexError::Config(format!(
            "unsupported asset extension: {ext}"
        )));
    };

    // Cache write is now only reached on cache miss — the cache-read gate above
    // short-circuits hits. The write path itself is unchanged (D-02, D-03).
    if let Err(e) = ensure_cache_parent(&cache_path) {
        tracing::debug!(error = %e, "asset cache mkdir skipped");
    } else if let Err(e) = tokio::fs::write(&cache_path, synthetic.as_bytes()).await {
        tracing::debug!(error = %e, path = %cache_path.display(), "asset cache write skipped");
    }

    chunk_markdown(&synthetic, asset_rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Minimal 1×1 PNG for cache-hit and standalone-image tests (transparent).
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[test]
    fn blockquote_single_line() {
        assert_eq!(
            blockquote_image("figure.png", "A chart"),
            "> **[Image: figure.png]** A chart"
        );
    }

    #[test]
    fn blockquote_multiline_prefixes_every_line() {
        assert_eq!(
            blockquote_image("figure.png", "line one\nline two\nline three"),
            "> **[Image: figure.png]** line one\n> line two\n> line three"
        );
    }

    #[test]
    fn blockquote_empty_description_still_emits_prefix() {
        assert_eq!(
            blockquote_image("pic.jpg", ""),
            "> **[Image: pic.jpg]** "
        );
    }

    #[tokio::test]
    async fn standalone_image_uses_blockquote_format() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "IMAGE_DESC"}],
                "id": "msg_1",
                "model": "claude",
                "role": "assistant",
                "stop_reason": "end_turn",
                "type": "message",
                "usage": {"input_tokens": 1, "output_tokens": 1}
            })))
            .mount(&server)
            .await;

        let vault = tempdir().unwrap();
        let img_path = vault.path().join("pic.png");
        tokio::fs::write(&img_path, PNG_1X1).await.unwrap();
        let data_dir = vault.path().join(".local-index");
        tokio::fs::create_dir_all(&data_dir).await.unwrap();

        let client = AnthropicAssetClient::new_for_test("test-key", server.uri());
        let cf = ingest_asset_path(
            vault.path(),
            Path::new("pic.png"),
            &data_dir,
            PNG_1X1.len() * 2,
            30,
            None,
            Some(&client),
        )
        .await
        .expect("ingest standalone image");

        assert!(
            cf.chunks
                .iter()
                .any(|c| c.body.contains("> **[Image: pic.png]** IMAGE_DESC")),
            "chunk body should contain blockquote-wrapped vision desc: {:?}",
            cf.chunks.first().map(|c| &c.body)
        );
    }

    #[tokio::test]
    async fn needsvision_pdf_pages_use_blockquote_format() {
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

        let vault = tempdir().unwrap();
        let pdf_path = vault.path().join("scan.pdf");
        let pdf_bytes = crate::pipeline::assets::pdf_local::fixture_needs_vision_single_page_pdf();
        tokio::fs::write(&pdf_path, &pdf_bytes).await.unwrap();
        let data_dir = vault.path().join(".local-index");
        tokio::fs::create_dir_all(&data_dir).await.unwrap();

        let client = AnthropicAssetClient::new_for_test("test-key", server.uri());
        let pdf_ocr = Some(OcrService::Anthropic(client.clone()));
        let cf = ingest_asset_path(
            vault.path(),
            Path::new("scan.pdf"),
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
                .any(|c| c.body.contains("> **[Image: scan_page_1.png]** OCR_PAGE_BODY")),
            "chunk body should contain blockquote-wrapped OCR page: {:?}",
            cf.chunks.first().map(|c| &c.body)
        );
    }

    #[tokio::test]
    async fn cache_hit_skips_api_and_returns_cached_synthetic() {
        let vault = tempdir().unwrap();
        let img_path = vault.path().join("pic.png");
        let bytes: &[u8] = b"PHASE11";
        tokio::fs::write(&img_path, bytes).await.unwrap();
        let data_dir = vault.path().join(".local-index");
        tokio::fs::create_dir_all(&data_dir).await.unwrap();

        // Pre-seed cache file for the source bytes' SHA-256 hash.
        let hex = sha256_hex(bytes);
        let cache_path = cache_path_for_hash(&data_dir, &hex);
        ensure_cache_parent(&cache_path).unwrap();
        tokio::fs::write(&cache_path, b"# cached\n\nCACHED_BODY\n")
            .await
            .unwrap();

        // image_vision: None would normally error for a .png; cache hit must short-circuit.
        let cf = ingest_asset_path(
            vault.path(),
            Path::new("pic.png"),
            &data_dir,
            bytes.len() * 2,
            30,
            None,
            None,
        )
        .await
        .expect("cache hit must short-circuit image_vision=None");

        assert!(
            cf.chunks.iter().any(|c| c.body.contains("CACHED_BODY")),
            "chunk body should contain cached synthetic: {:?}",
            cf.chunks.first().map(|c| &c.body)
        );
    }

    #[tokio::test]
    async fn corrupt_cache_triggers_refetch() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "FRESH_DESC"}],
                "id": "msg_1",
                "model": "claude",
                "role": "assistant",
                "stop_reason": "end_turn",
                "type": "message",
                "usage": {"input_tokens": 1, "output_tokens": 1}
            })))
            .mount(&server)
            .await;

        let vault = tempdir().unwrap();
        let img_path = vault.path().join("pic.png");
        tokio::fs::write(&img_path, PNG_1X1).await.unwrap();
        let data_dir = vault.path().join(".local-index");
        tokio::fs::create_dir_all(&data_dir).await.unwrap();

        // Pre-seed an EMPTY cache file — must be treated as corrupt and refetched.
        let hex = sha256_hex(PNG_1X1);
        let cache_path = cache_path_for_hash(&data_dir, &hex);
        ensure_cache_parent(&cache_path).unwrap();
        tokio::fs::write(&cache_path, b"").await.unwrap();

        let client = AnthropicAssetClient::new_for_test("test-key", server.uri());
        let cf = ingest_asset_path(
            vault.path(),
            Path::new("pic.png"),
            &data_dir,
            PNG_1X1.len() * 2,
            30,
            None,
            Some(&client),
        )
        .await
        .expect("corrupt cache should refetch and succeed");

        assert!(
            cf.chunks.iter().any(|c| c.body.contains("FRESH_DESC")),
            "fresh desc must appear after corrupt-cache refetch: {:?}",
            cf.chunks.first().map(|c| &c.body)
        );
        let reqs = server.received_requests().await.unwrap();
        assert!(
            !reqs.is_empty(),
            "corrupt cache should have triggered at least one API request"
        );
    }

    #[tokio::test]
    async fn text_first_pdf_chunks_use_asset_path() {
        let vault = tempdir().unwrap();
        let pdf_path = vault.path().join("doc.pdf");
        let pdf_bytes = crate::pipeline::assets::pdf_local::fixture_single_page_text_pdf();
        tokio::fs::write(&pdf_path, &pdf_bytes).await.unwrap();
        let rel = Path::new("doc.pdf");
        let data_dir = vault.path().join(".local-index");
        tokio::fs::create_dir_all(&data_dir).await.unwrap();
        let cf = ingest_asset_path(
            vault.path(),
            rel,
            &data_dir,
            pdf_bytes.len() * 2,
            30,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(
            cf.chunks.iter().all(|c| c.file_path == PathBuf::from("doc.pdf")),
            "chunk file_path should be vault-relative asset path"
        );
        assert!(cf.chunks.iter().any(|c| c.body.contains("PHASE09_FIXTURE")));
    }

    /// Returns true when the pdfium system library can be bound — required for
    /// TextFirst embedded-image extraction. Mirrors the graceful-degradation
    /// contract of [`super::pdf_images::extract_embedded_images_per_page`] so
    /// this test suite stays portable on machines where only Poppler
    /// (`pdftoppm`) is available.
    fn pdfium_available() -> bool {
        pdfium_render::prelude::Pdfium::bind_to_system_library().is_ok()
    }

    #[tokio::test]
    async fn textfirst_pdf_interleaves_text_and_image_blockquotes_per_page() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "EMBED_DESC"}],
                "id": "msg_1",
                "model": "claude",
                "role": "assistant",
                "stop_reason": "end_turn",
                "type": "message",
                "usage": {"input_tokens": 1, "output_tokens": 1}
            })))
            .mount(&server)
            .await;

        let vault = tempdir().unwrap();
        let pdf_bytes =
            crate::pipeline::assets::pdf_local::fixture_single_page_pdf_with_embedded_image();
        let pdf_path = vault.path().join("doc.pdf");
        tokio::fs::write(&pdf_path, &pdf_bytes).await.unwrap();
        let rel = Path::new("doc.pdf");
        let data_dir = vault.path().join(".local-index");
        tokio::fs::create_dir_all(&data_dir).await.unwrap();

        let client = AnthropicAssetClient::new_for_test("test-key", server.uri());
        let cf = ingest_asset_path(
            vault.path(),
            rel,
            &data_dir,
            pdf_bytes.len() * 2,
            30,
            None,
            Some(&client),
        )
        .await
        .expect("ingest textfirst pdf with embedded image");

        let joined: String = cf
            .chunks
            .iter()
            .map(|c| c.body.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("## Page 1"),
            "expected ## Page 1 heading; got: {joined}"
        );
        assert!(
            joined.contains("PHASE11_TEXT_AND_IMAGE"),
            "expected page text token; got: {joined}"
        );
        if pdfium_available() {
            // When pdfium binds, the fixture's embedded PNG is extracted and
            // sent through describe_image — the mock returns EMBED_DESC,
            // wrapped via `blockquote_image`.
            assert!(
                joined.contains("> **[Image: doc_page_1_image_1.png]** EMBED_DESC"),
                "expected embedded image blockquote; got: {joined}"
            );
        } else {
            // Graceful degradation contract (research Pitfall 1): when
            // libpdfium is not available, embedded-image extraction yields an
            // empty vec + WARN; text still indexes but no image blockquote is
            // emitted. This mirrors `extracts_empty_page_list_from_text_only_pdf`
            // in pdf_images::tests.
            assert!(
                !joined.contains("> **[Image: doc_page_1_image_1.png]**"),
                "no blockquote expected when pdfium is unavailable; got: {joined}"
            );
        }
    }

    #[tokio::test]
    async fn textfirst_pdf_without_vision_client_warns_and_indexes_text_only() {
        let vault = tempdir().unwrap();
        let pdf_bytes =
            crate::pipeline::assets::pdf_local::fixture_single_page_pdf_with_embedded_image();
        let pdf_path = vault.path().join("doc.pdf");
        tokio::fs::write(&pdf_path, &pdf_bytes).await.unwrap();
        let rel = Path::new("doc.pdf");
        let data_dir = vault.path().join(".local-index");
        tokio::fs::create_dir_all(&data_dir).await.unwrap();

        let cf = ingest_asset_path(
            vault.path(),
            rel,
            &data_dir,
            pdf_bytes.len() * 2,
            30,
            None,
            None, // no vision client — graceful degradation
        )
        .await
        .expect("ingest textfirst pdf without vision should still succeed");

        let joined: String = cf
            .chunks
            .iter()
            .map(|c| c.body.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("PHASE11_TEXT_AND_IMAGE"),
            "page text must still be indexed: {joined}"
        );
        assert!(
            !joined.contains("> **[Image: doc_page_"),
            "no image blockquote should be emitted without a vision client: {joined}"
        );
    }

    #[tokio::test]
    async fn needs_vision_pdf_routes_raster_pages_through_ocr_service() {
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

        let vault = tempdir().unwrap();
        let pdf_path = vault.path().join("scan.pdf");
        let pdf_bytes = crate::pipeline::assets::pdf_local::fixture_needs_vision_single_page_pdf();
        tokio::fs::write(&pdf_path, &pdf_bytes).await.unwrap();
        let rel = Path::new("scan.pdf");
        let data_dir = vault.path().join(".local-index");
        tokio::fs::create_dir_all(&data_dir).await.unwrap();

        let client =
            AnthropicAssetClient::new_for_test("test-key", server.uri());
        let pdf_ocr = Some(OcrService::Anthropic(client.clone()));
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
            "chunk body should include mocked OCR text: {:?}",
            cf.chunks.first().map(|c| &c.body)
        );
    }
}
