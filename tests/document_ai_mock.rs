//! Wiremock proof for Google Document AI `:process` JSON (`10-02`).

use local_index::pipeline::assets::{DocumentAiClient, OcrService};
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Minimal 1×1 PNG (transparent).
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

#[tokio::test]
async fn google_ocr_process_returns_document_text() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_regex(r".*:process$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "document": { "text": "OCR_PAGE_OK" }
        })))
        .mount(&server)
        .await;

    let client = DocumentAiClient::new_for_test(
        "test-project",
        "us",
        "proc-1",
        server.uri(),
        "test-bearer-token",
    );

    let svc = OcrService::Google(client);
    let pages = vec![PNG_1X1.to_vec()];
    let out = svc
        .ocr_scanned_pdf_pages(&pages)
        .await
        .expect("ocr_scanned_pdf_pages");

    assert_eq!(out.len(), 1);
    assert!(
        out[0].contains("OCR_PAGE_OK"),
        "unexpected OCR text: {:?}",
        out[0]
    );
}
