//! Local PDF classification and text extraction (no network).

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};

use crate::error::LocalIndexError;

/// Result of the text-density heuristic for a PDF (`PRE-05`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfClassification {
    /// Enough decoded text per page to treat as text-first (`PRE-06` path).
    TextFirst,
    /// Sparse or missing text — requires raster + vision (later preprocessor phases).
    NeedsVision,
}

fn ensure_under_cap(bytes: &[u8], max_bytes: usize) -> Result<(), LocalIndexError> {
    if bytes.len() > max_bytes {
        return Err(LocalIndexError::AssetTooLarge {
            bytes: bytes.len(),
            max_bytes,
        });
    }
    Ok(())
}

/// Count characters we treat as “printable” for the text-density heuristic.
///
/// Control characters are excluded except common whitespace (`PRE-05` / `PRE-06`).
fn printable_char_count(text: &str) -> usize {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .count()
}

/// Classify a PDF using a local text-density heuristic (`PRE-05`).
///
/// **Heuristic:** let `page_count` be the number of pages (minimum 1). If the sum of printable
/// characters extracted across all pages is at least **`12 * page_count`**, classify as
/// [`PdfClassification::TextFirst`]; otherwise [`PdfClassification::NeedsVision`].
pub fn classify_pdf(bytes: &[u8], max_bytes: usize) -> Result<PdfClassification, LocalIndexError> {
    ensure_under_cap(bytes, max_bytes)?;
    let doc = Document::load_mem(bytes).map_err(LocalIndexError::Pdf)?;
    let page_numbers: Vec<u32> = doc.get_pages().keys().cloned().collect();
    let page_count = page_numbers.len().max(1);
    let mut total_printable = 0usize;
    for pn in &page_numbers {
        let page_text = doc.extract_text(&[*pn]).map_err(LocalIndexError::Pdf)?;
        total_printable = total_printable.saturating_add(printable_char_count(&page_text));
    }
    let threshold = 12usize.saturating_mul(page_count);
    Ok(if total_printable >= threshold {
        PdfClassification::TextFirst
    } else {
        PdfClassification::NeedsVision
    })
}

/// Extract page text locally and wrap as light markdown suitable for [`crate::pipeline::chunker::chunk_markdown`] (`PRE-06`).
///
/// **Note on visibility:** preserved as `pub` per Plan 09-01/11-02 contract
/// even though no caller in the crate currently uses it — Phase 11-02
/// replaced the former call site in `ingest.rs` with a per-page loop
/// (`extract_page_text_vec` + `extract_embedded_images_per_page`). Downstream
/// consumers (future phases, CLI probes) may still need this flat-markdown
/// form, so the export is kept alive with `#[allow(dead_code)]`.
#[allow(dead_code)]
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

/// Extract per-page text locally, preserving page index alignment (`PRE-10`).
///
/// Returns a `Vec<String>` with one entry per page up to `max_pages`:
/// empty pages contribute `String::new()` so callers can align the result
/// with per-page image extraction from [`crate::pipeline::assets::pdf_images`].
pub fn extract_page_text_vec(
    bytes: &[u8],
    max_bytes: usize,
    max_pages: usize,
) -> Result<Vec<String>, LocalIndexError> {
    ensure_under_cap(bytes, max_bytes)?;
    if max_pages == 0 {
        return Ok(Vec::new());
    }
    let doc = Document::load_mem(bytes).map_err(LocalIndexError::Pdf)?;
    let page_numbers: Vec<u32> = doc.get_pages().keys().cloned().collect();
    let mut out: Vec<String> = Vec::new();
    for (i, pn) in page_numbers.into_iter().enumerate() {
        if i >= max_pages {
            break;
        }
        let page_text = doc.extract_text(&[pn]).map_err(LocalIndexError::Pdf)?;
        out.push(page_text.trim().to_string());
    }
    Ok(out)
}

/// Single-page PDF with visible text `PHASE09_FIXTURE` (Courier), for tests and raster fixtures.
#[cfg(test)]
pub(crate) fn fixture_single_page_text_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("phase09 test"),
        "Creator" => Object::string_literal("local-index tests"),
    });
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });
    let text = "PHASE09_FIXTURE";
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 48.into()]),
            Operation::new("Td", vec![100.into(), 600.into()]),
            Operation::new("Tj", vec![Object::string_literal(text)]),
            Operation::new("ET", vec![]),
        ],
    };
    let pages: Vec<Object> = [content]
        .into_iter()
        .map(|content| {
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
            });
            page.into()
        })
        .collect();

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => pages,
        "Count" => 1,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.trailer.set("Info", info_id);
    doc.compress();
    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("save pdf");
    buf
}

/// Single-page PDF with one embedded 1×1 PNG image and minimal text,
/// for testing TextFirst embedded-image extraction (`PRE-09`, `PRE-10`).
///
/// **Note on visibility:** this fixture is `pub fn` (not `#[cfg(test)]`) so
/// it can be reached from integration tests in `tests/` via
/// [`crate::test_support`]. The PDF it produces is tiny (~1 KB) and is
/// unreachable from production call sites; shipping it in the release
/// binary is acceptable overhead.
pub fn fixture_single_page_pdf_with_embedded_image() -> Vec<u8> {
    // Minimal 1×1 RGBA PNG (transparent) — 67 bytes.
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    let mut doc = Document::with_version("1.5");
    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("phase11 embedded image fixture"),
        "Creator" => Object::string_literal("local-index tests"),
    });
    let pages_id = doc.new_object_id();

    // Font for the minimum-text requirement that forces TextFirst classification.
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });

    // Embedded image XObject (PNG bytes; pdfium-render happily decodes
    // /Filter-less streams via get_raw_image by recognizing the IHDR
    // signature).
    let image_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 1,
            "Height" => 1,
            "BitsPerComponent" => 8,
            "ColorSpace" => "DeviceRGB",
        },
        PNG_1X1.to_vec(),
    );
    let image_id = doc.add_object(image_stream);

    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
        "XObject" => dictionary! { "Im1" => image_id },
    });

    // Content stream: draw a tiny bit of text (so classification comes out
    // TextFirst) and invoke /Im1 via /Do.
    let text = "PHASE11_TEXT_AND_IMAGE";
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 48.into()]),
            Operation::new("Td", vec![50.into(), 700.into()]),
            Operation::new("Tj", vec![Object::string_literal(text)]),
            Operation::new("ET", vec![]),
            // Transform + draw the image: `50 0 0 50 100 500 cm /Im1 Do`
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    50.into(),
                    0.into(),
                    0.into(),
                    50.into(),
                    100.into(),
                    500.into(),
                ],
            ),
            Operation::new("Do", vec!["Im1".into()]),
            Operation::new("Q", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
    });
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::from(page)],
        "Count" => 1,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.trailer.set("Info", info_id);
    doc.compress();
    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("save pdf");
    buf
}

/// Single-page PDF with essentially no extractable text — classifies as [`PdfClassification::NeedsVision`].
#[cfg(test)]
pub(crate) fn fixture_needs_vision_single_page_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("needs-vision fixture"),
        "Creator" => Object::string_literal("local-index tests"),
    });
    let pages_id = doc.new_object_id();
    let resources_id = doc.add_object(dictionary! {});
    let content = Content { operations: vec![] };
    let pages: Vec<Object> = [content]
        .into_iter()
        .map(|content| {
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
            });
            page.into()
        })
        .collect();

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => pages,
        "Count" => 1,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.trailer.set("Info", info_id);
    doc.compress();
    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("save pdf");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_text_first_fixture_pdf() {
        let bytes = fixture_single_page_text_pdf();
        assert_eq!(
            classify_pdf(&bytes, bytes.len()).unwrap(),
            PdfClassification::TextFirst
        );
    }

    #[test]
    fn classify_needs_vision_sparse_fixture_pdf() {
        let bytes = fixture_needs_vision_single_page_pdf();
        assert_eq!(
            classify_pdf(&bytes, bytes.len()).unwrap(),
            PdfClassification::NeedsVision
        );
    }

    #[test]
    fn extract_markdown_contains_fixture_token() {
        let bytes = fixture_single_page_text_pdf();
        let md = extract_text_pdf_as_markdown(&bytes, bytes.len()).unwrap();
        assert!(
            md.contains("PHASE09_FIXTURE"),
            "markdown missing fixture token: {md:?}"
        );
    }

    #[test]
    fn asset_too_large_returns_error_message() {
        let bytes = vec![0u8; 16];
        let err = classify_pdf(&bytes, 8).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("asset too large"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn extract_page_text_vec_returns_one_entry_per_page_with_fixture() {
        let bytes = fixture_single_page_text_pdf();
        let pages = extract_page_text_vec(&bytes, bytes.len(), 30).unwrap();
        assert_eq!(pages.len(), 1, "expected one page");
        assert!(
            pages[0].contains("PHASE09_FIXTURE"),
            "page 0 missing fixture token: {:?}",
            pages[0]
        );
    }

    #[test]
    fn extract_page_text_vec_returns_empty_string_for_empty_page() {
        let bytes = fixture_needs_vision_single_page_pdf();
        let pages = extract_page_text_vec(&bytes, bytes.len(), 30).unwrap();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].is_empty(), "expected empty string for empty page");
    }

    #[test]
    fn extract_page_text_vec_respects_max_pages_cap() {
        let bytes = fixture_single_page_text_pdf();
        let pages = extract_page_text_vec(&bytes, bytes.len(), 0).unwrap();
        assert!(pages.is_empty());
    }

    #[test]
    fn extract_page_text_vec_respects_max_bytes_cap() {
        let bytes = fixture_single_page_text_pdf();
        let err = extract_page_text_vec(&bytes, 8, 30).unwrap_err();
        assert!(err.to_string().contains("asset too large"));
    }

    #[test]
    fn fixture_single_page_pdf_with_embedded_image_classifies_textfirst() {
        let bytes = fixture_single_page_pdf_with_embedded_image();
        assert_eq!(
            classify_pdf(&bytes, bytes.len()).unwrap(),
            PdfClassification::TextFirst,
            "new embedded-image fixture must classify as TextFirst so the TextFirst \
             branch of ingest_asset_path exercises embedded-image extraction"
        );
    }
}
