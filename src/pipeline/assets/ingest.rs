//! Orchestrate asset → synthetic markdown → [`crate::pipeline::chunker::chunk_markdown`] (`PRE-01` wiring).

use std::path::Path;

use sha2::{Digest, Sha256};

use super::anthropic_extract::AnthropicAssetClient;
use super::cache::{cache_path_for_hash, ensure_cache_parent};
use super::pdf_local::{classify_pdf, extract_text_pdf_as_markdown, PdfClassification};
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

/// Ingest a single vault-relative asset path: classify, extract locally and/or call vision, then chunk.
///
/// `Chunk.file_path` in the returned [`ChunkedFile`] is always `asset_rel` (the original asset).
pub async fn ingest_asset_path(
    vault: &Path,
    asset_rel: &Path,
    data_dir: &Path,
    max_bytes: usize,
    max_pdf_pages: usize,
    client: Option<&AnthropicAssetClient>,
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
                let body = extract_text_pdf_as_markdown(&bytes, max_bytes)?;
                format!("# Source: {fname}\n\n{body}")
            }
            PdfClassification::NeedsVision => {
                let Some(c) = client else {
                    return Err(LocalIndexError::Credential(
                        "ANTHROPIC_API_KEY is required for scanned PDFs (NeedsVision). \
                         Set ANTHROPIC_API_KEY from https://console.anthropic.com/"
                            .to_string(),
                    ));
                };
                let pngs = rasterize_pdf_pages_to_png(&bytes, max_pdf_pages)?;
                if pngs.is_empty() {
                    return Err(LocalIndexError::Config(
                        "no pages rasterized from PDF".to_string(),
                    ));
                }
                let mut parts = Vec::new();
                for png in &pngs {
                    let desc = c.describe_raster_page(png).await?;
                    parts.push(desc);
                }
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
        let Some(c) = client else {
            return Err(LocalIndexError::Credential(
                "ANTHROPIC_API_KEY is required for image assets. \
                 Set ANTHROPIC_API_KEY from https://console.anthropic.com/"
                    .to_string(),
            ));
        };
        let desc = c.describe_image(mt, &bytes).await?;
        format!("# {fname}\n\n{desc}")
    } else {
        return Err(LocalIndexError::Config(format!(
            "unsupported asset extension: {ext}"
        )));
    };

    // Optional cache write (debug / retry aid) — does not affect chunk provenance.
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let cache_path = cache_path_for_hash(data_dir, &hex);
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
        )
        .await
        .unwrap();
        assert!(
            cf.chunks.iter().all(|c| c.file_path == PathBuf::from("doc.pdf")),
            "chunk file_path should be vault-relative asset path"
        );
        assert!(cf.chunks.iter().any(|c| c.body.contains("PHASE09_FIXTURE")));
    }
}
