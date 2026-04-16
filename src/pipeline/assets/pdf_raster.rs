//! Rasterize PDF pages to PNG bytes for Anthropic vision (`D-05`).
//!
//! Uses **PDFium** via [`pdfium_render`] when a system `libpdfium` can be loaded; otherwise falls
//! back to the **`pdftoppm`** CLI from [Poppler](https://poppler.freedesktop.org/) if it is on
//! `PATH`. Install one of: a Pdfium shared library discoverable by dynamic link, or
//! `poppler-utils` (Debian/Ubuntu: `apt install poppler-utils`, macOS: `brew install poppler`).
//!
//! Page cap for vision calls comes from env **`LOCAL_INDEX_MAX_PDF_PAGES`** (default **30**) at
//! orchestration sites (e.g. `ingest.rs` / `main.rs`).

use std::io::Cursor;
use std::path::Path;
use std::process::Command;

use image::ImageFormat;
use pdfium_render::prelude::*;

use crate::error::LocalIndexError;

/// Rasterize the first `max_pages` pages of `pdf_bytes` to PNG images (one buffer per page).
///
/// Returns an empty vec only when rasterization produced no pages (treated as an error by callers).
pub fn rasterize_pdf_pages_to_png(
    pdf_bytes: &[u8],
    max_pages: usize,
) -> Result<Vec<Vec<u8>>, LocalIndexError> {
    if max_pages == 0 {
        return Err(LocalIndexError::Config(
            "max_pages must be > 0 (check LOCAL_INDEX_MAX_PDF_PAGES)".to_string(),
        ));
    }

    if let Some(pngs) = try_pdfium(pdf_bytes, max_pages) {
        if !pngs.is_empty() {
            return Ok(pngs);
        }
    }

    if let Ok(pngs) = try_pdftoppm(pdf_bytes, max_pages) {
        if !pngs.is_empty() {
            return Ok(pngs);
        }
    }

    Err(LocalIndexError::Config(
        "PDF rasterization failed: could not load Pdfium (system library) and `pdftoppm` \
         is missing or failed. Install Poppler (`pdftoppm`) or provide a Pdfium shared library."
            .to_string(),
    ))
}

fn try_pdfium(pdf_bytes: &[u8], max_pages: usize) -> Option<Vec<Vec<u8>>> {
    let bindings = Pdfium::bind_to_system_library().ok()?;
    let pdfium = Pdfium::new(bindings);
    let doc = pdfium.load_pdf_from_byte_slice(pdf_bytes, None).ok()?;

    let render_config = PdfRenderConfig::new()
        .set_target_width(1024)
        .set_maximum_height(1024);

    let mut out = Vec::new();
    for (idx, page) in doc.pages().iter().enumerate() {
        if idx >= max_pages {
            break;
        }
        let image = page.render_with_config(&render_config).ok()?.as_image();
        let mut buf = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .ok()?;
        out.push(buf);
    }
    Some(out)
}

fn try_pdftoppm(pdf_bytes: &[u8], max_pages: usize) -> Result<Vec<Vec<u8>>, LocalIndexError> {
    let dir = tempfile::tempdir().map_err(LocalIndexError::Io)?;
    let pdf_path = dir.path().join("doc.pdf");
    std::fs::write(&pdf_path, pdf_bytes)?;
    let out_base = dir.path().join("page");
    let pdf_s = pdf_path.to_str().ok_or_else(|| {
        LocalIndexError::Config("pdftoppm: invalid temp pdf path".to_string())
    })?;
    let out_s = out_base.to_str().ok_or_else(|| {
        LocalIndexError::Config("pdftoppm: invalid temp out path".to_string())
    })?;

    let status = Command::new("pdftoppm")
        .args([
            "-png",
            "-f",
            "1",
            "-l",
            &max_pages.to_string(),
            pdf_s,
            out_s,
        ])
        .status()
        .map_err(|_| LocalIndexError::Config("pdftoppm: not found on PATH".to_string()))?;

    if !status.success() {
        return Err(LocalIndexError::Config(
            "pdftoppm: subprocess failed".to_string(),
        ));
    }

    let mut pages = Vec::new();
    for i in 1..=max_pages {
        let p = format!("{}-{}.png", out_s, i);
        let path = Path::new(&p);
        if path.exists() {
            pages.push(std::fs::read(path).map_err(LocalIndexError::Io)?);
        } else {
            break;
        }
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::assets::pdf_local::fixture_single_page_text_pdf;

    #[test]
    fn max_pages_zero_errors() {
        let pdf = fixture_single_page_text_pdf();
        let err = rasterize_pdf_pages_to_png(&pdf, 0).unwrap_err();
        assert!(err.to_string().contains("max_pages"));
    }

    #[test]
    fn rasterizes_fixture_when_backend_available() {
        let pdf = fixture_single_page_text_pdf();
        let res = rasterize_pdf_pages_to_png(&pdf, 1);
        match res {
            Ok(v) => assert_eq!(v.len(), 1, "expected one PNG page"),
            Err(e) => panic!(
                "rasterize failed (install poppler or pdfium): {e}"
            ),
        }
    }
}
