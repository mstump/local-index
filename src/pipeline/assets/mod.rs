//! Asset preprocessing (PDF / raster images).
//!
//! Synthetic markdown produced from assets is fed through [`crate::pipeline::chunker::chunk_markdown`]
//! with the **source** asset [`Path`] for provenance.

mod anthropic_extract;
mod cache;
mod ignore_walk;
mod ingest;
mod pdf_local;
mod pdf_raster;

pub use anthropic_extract::{AnthropicAssetClient, ASSET_VISION_PROMPT};

pub use ingest::ingest_asset_path;

pub use ignore_walk::discover_asset_paths;
pub(crate) use ignore_walk::{build_asset_exclude_override, is_asset_path_excluded_by_override};
