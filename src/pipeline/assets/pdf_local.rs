//! Local PDF classification and text extraction (no network).

use crate::error::LocalIndexError;

/// Result of the text-density heuristic for a PDF (`PRE-05`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfClassification {
    /// Enough decoded text per page to treat as text-first (`PRE-06` path).
    TextFirst,
    /// Sparse or missing text — requires raster + vision (`PRE` later phases).
    NeedsVision,
}

/// Placeholder until Task 3.
pub fn classify_pdf(_bytes: &[u8], _max_bytes: usize) -> Result<PdfClassification, LocalIndexError> {
    Ok(PdfClassification::NeedsVision)
}

/// Placeholder until Task 3.
pub fn extract_text_pdf_as_markdown(
    _bytes: &[u8],
    _max_bytes: usize,
) -> Result<String, LocalIndexError> {
    Ok(String::new())
}
