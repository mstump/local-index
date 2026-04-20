//! Local PDF classification and text extraction (no network).

use lopdf::Document;

#[cfg(test)]
use lopdf::content::{Content, Operation};
#[cfg(test)]
use lopdf::{dictionary, Object, Stream};

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
