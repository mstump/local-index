//! Extract embedded raster images from PDF pages via `pdfium-render` for TextFirst vision enrichment (`PRE-09`, `PRE-10`, `D-07`/`D-08`/`D-09`).

use crate::error::LocalIndexError;

/// RED stub — implementation in GREEN commit.
pub fn extract_embedded_images_per_page(
    _pdf_bytes: &[u8],
    _max_pages: usize,
) -> Result<Vec<Vec<Vec<u8>>>, LocalIndexError> {
    // Force failure — not implemented yet.
    Err(LocalIndexError::Config("not implemented (RED)".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_empty_vec_when_max_pages_zero() {
        let result = extract_embedded_images_per_page(b"any", 0).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn returns_empty_vec_when_pdf_invalid() {
        // Not a PDF — pdfium load fails → graceful degradation path.
        let result = extract_embedded_images_per_page(b"not a pdf", 10).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn extracts_empty_page_list_from_text_only_pdf() {
        // Phase 9 fixture has one page with text and no embedded images.
        let pdf = crate::pipeline::assets::pdf_local::fixture_single_page_text_pdf();
        let result = extract_embedded_images_per_page(&pdf, 10).unwrap();
        // When pdfium is available, one page with zero images: vec![vec![]].
        // When pdfium is NOT available, graceful degradation: vec![].
        // Either is acceptable; the assertion covers both shapes.
        assert!(
            result.is_empty() || (result.len() == 1 && result[0].is_empty()),
            "expected empty or single-empty-page result, got {:?}",
            result.iter().map(|v| v.len()).collect::<Vec<_>>()
        );
    }
}
