//! Extract embedded raster images from PDF pages via `pdfium-render` for TextFirst vision enrichment (`PRE-09`, `PRE-10`, `D-07`/`D-08`/`D-09`).
//!
//! Uses [`PdfPageImageObject::get_raw_image`] which returns a decoded
//! [`image::DynamicImage`] that we re-encode as PNG. Never uses
//! [`PdfPageImageObject::get_raw_image_data`] because that returns
//! filter-encoded bytes (DCTDecode/JPXDecode/FlateDecode/CCITTFaxDecode) with
//! no reliable way to detect the IANA media type for Anthropic vision.
//!
//! When the system `libpdfium` is not available, returns an empty `Vec`
//! plus a `tracing::warn!` — TextFirst PDFs still index their text in that
//! case (graceful degradation, research Pitfall 1).

use std::io::Cursor;

use image::ImageFormat;
use pdfium_render::prelude::*;

use crate::error::LocalIndexError;

/// Extract PNG byte buffers for every embedded image on each page of a PDF.
///
/// Returns a nested `Vec` indexed by page then image: `out[page][image]` is
/// the PNG bytes of the i-th embedded image on the p-th page. Pages with no
/// embedded images contribute an empty inner `Vec`.
///
/// Iteration is capped at `max_pages` to match the rasterization cap.
///
/// Errors: returns `LocalIndexError::Config` only on pdfium load failure for
/// a byte buffer that looked valid (pdfium binding returned but parse failed).
/// A missing system library is **not** an error — it produces `Ok(vec![])`
/// plus a WARN, matching the `try_pdfium` graceful-degradation pattern in
/// `pdf_raster.rs`.
pub fn extract_embedded_images_per_page(
    pdf_bytes: &[u8],
    max_pages: usize,
) -> Result<Vec<Vec<Vec<u8>>>, LocalIndexError> {
    if max_pages == 0 {
        return Ok(Vec::new());
    }

    let Ok(bindings) = Pdfium::bind_to_system_library() else {
        tracing::warn!(
            "pdfium system library not available; embedded-image vision \
             skipped for this PDF. Install libpdfium to enable TextFirst PDF \
             embedded-image descriptions."
        );
        return Ok(Vec::new());
    };
    let pdfium = Pdfium::new(bindings);
    let doc = match pdfium.load_pdf_from_byte_slice(pdf_bytes, None) {
        Ok(doc) => doc,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "pdfium could not load PDF for embedded-image extraction; \
                 indexing text only"
            );
            return Ok(Vec::new());
        }
    };

    let mut pages_out: Vec<Vec<Vec<u8>>> = Vec::new();
    for (page_idx, page) in doc.pages().iter().enumerate() {
        if page_idx >= max_pages {
            break;
        }
        let mut page_images: Vec<Vec<u8>> = Vec::new();
        for object in page.objects().iter() {
            let Some(img_obj) = object.as_image_object() else {
                continue;
            };
            // `get_raw_image` returns a decoded DynamicImage — safe to
            // re-encode to PNG. `get_raw_image_data` returns filter-encoded
            // bytes with no way to detect media_type for Anthropic.
            let Ok(dyn_image) = img_obj.get_raw_image() else {
                // Image masks, soft masks, and transform-heavy objects can
                // fail to decode; skip them silently (Pitfall 2 in research).
                continue;
            };
            let mut buf: Vec<u8> = Vec::new();
            if dyn_image
                .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
                .is_ok()
            {
                page_images.push(buf);
            }
        }
        pages_out.push(page_images);
    }
    Ok(pages_out)
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
