//! Asset preprocessing (PDF / raster images).
//!
//! Synthetic markdown produced from assets is fed through [`crate::pipeline::chunker::chunk_markdown`]
//! with the **source** asset [`Path`] for provenance.

mod anthropic_extract;
mod cache;
mod document_ai;
mod ignore_walk;
mod ingest;
mod ocr;
mod pdf_images;
mod pdf_local;
mod pdf_raster;

pub use anthropic_extract::{AnthropicAssetClient, ASSET_VISION_PROMPT};
pub use document_ai::DocumentAiClient;
pub use ocr::OcrService;

pub use ingest::ingest_asset_path;

use crate::credentials::OcrProvider;
use crate::error::LocalIndexError;

/// Build PDF OCR service and optional Anthropic client for standalone images based on [`OcrProvider`].
pub fn build_ocr_and_image_clients(
    provider: OcrProvider,
) -> Result<(Option<OcrService>, Option<AnthropicAssetClient>), LocalIndexError> {
    match provider {
        OcrProvider::Anthropic => {
            let image = AnthropicAssetClient::new_from_env().ok();
            let pdf = image.clone().map(OcrService::Anthropic);
            Ok((pdf, image))
        }
        OcrProvider::Google => {
            let doc = DocumentAiClient::new_from_env()?;
            let pdf = Some(OcrService::Google(doc));
            let image = AnthropicAssetClient::new_from_env().ok();
            Ok((pdf, image))
        }
    }
}

pub use ignore_walk::discover_asset_paths;
pub(crate) use ignore_walk::{build_asset_exclude_override, is_asset_path_excluded_by_override};
