//! Asset preprocessing (PDF / raster images).
//!
//! Synthetic markdown produced from assets is fed through [`crate::pipeline::chunker::chunk_markdown`]
//! with the **source** asset [`Path`] for provenance (wired in Plan 09-03).

#![allow(dead_code, unused_imports)] // facade consumed by Plans 09-02 / 09-03

mod cache;
mod ignore_walk;
mod pdf_local;

pub(crate) use cache::{cache_dir, cache_path_for_hash, ensure_cache_parent};
pub(crate) use ignore_walk::discover_asset_paths;
pub(crate) use pdf_local::{classify_pdf, extract_text_pdf_as_markdown, PdfClassification};
