//! OCR dispatch for **rasterized scanned PDF pages** only (`Phase 10`, CONTEXT D-01).
//!
//! Standalone raster images (`png`, `jpg`, `jpeg`, `webp`) are **not** routed through this module;
//! they continue to use [`AnthropicAssetClient::describe_image`] directly from [`super::ingest`].

use super::anthropic_extract::AnthropicAssetClient;
use super::document_ai::DocumentAiClient;
use crate::error::LocalIndexError;

/// Pluggable OCR for the NeedsVision PDF path (per-page PNG buffers after rasterization).
#[derive(Clone)]
pub enum OcrService {
    Anthropic(AnthropicAssetClient),
    Google(DocumentAiClient),
}

impl OcrService {
    /// Run OCR on each rasterized PDF page; order matches `png_pages`.
    pub async fn ocr_scanned_pdf_pages(
        &self,
        png_pages: &[Vec<u8>],
    ) -> Result<Vec<String>, LocalIndexError> {
        match self {
            OcrService::Anthropic(client) => {
                let mut out = Vec::with_capacity(png_pages.len());
                for png in png_pages {
                    out.push(client.describe_raster_page(png).await?);
                }
                Ok(out)
            }
            OcrService::Google(client) => {
                let mut out = Vec::with_capacity(png_pages.len());
                for png in png_pages {
                    out.push(client.process_png_page(png).await?);
                }
                Ok(out)
            }
        }
    }
}
