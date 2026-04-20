//! Wiremock proof for Anthropic asset vision JSON (`09-02`).

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use local_index::pipeline::assets::AnthropicAssetClient;
use wiremock::matchers::{method, path};
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
async fn describe_image_posts_messages_contract() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{"type": "text", "text": "OK_DESCRIPTION"}],
            "id": "msg_1",
            "model": "claude",
            "role": "assistant",
            "stop_reason": "end_turn",
            "type": "message",
            "usage": {"input_tokens": 1, "output_tokens": 1}
        })))
        .mount(&server)
        .await;

    let client = AnthropicAssetClient::new_for_test("test-api-key", server.uri());

    let out = client
        .describe_image("image/png", PNG_1X1)
        .await
        .expect("describe_image");
    assert_eq!(out, "OK_DESCRIPTION");

    // Verify request contained our fixed prompt and base64 payload
    let reqs = server.received_requests().await.unwrap();
    let last = reqs.last().expect("one request");
    let body: serde_json::Value = serde_json::from_slice(&last.body).unwrap();
    let content = &body["messages"][0]["content"];
    let arr = content.as_array().unwrap();
    let img = arr.iter().find(|b| b["type"] == "image").unwrap();
    let data = img["source"]["data"].as_str().unwrap();
    assert_eq!(data, B64.encode(PNG_1X1));
    let txt = arr.iter().find(|b| b["type"] == "text").unwrap();
    assert_eq!(
        txt["text"].as_str().unwrap(),
        local_index::pipeline::assets::ASSET_VISION_PROMPT
    );
}

#[tokio::test]
async fn textfirst_pdf_calls_vision_per_embedded_image() {
    use local_index::pipeline::assets::ingest_asset_path;
    use std::path::Path;
    use tempfile::tempdir;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{"type": "text", "text": "EMBEDDED_IMG_DESC"}],
            "id": "msg_1",
            "model": "claude",
            "role": "assistant",
            "stop_reason": "end_turn",
            "type": "message",
            "usage": {"input_tokens": 1, "output_tokens": 1}
        })))
        .mount(&server)
        .await;

    let vault = tempdir().unwrap();
    let pdf_bytes = local_index::test_support::fixture_single_page_pdf_with_embedded_image();
    let pdf_path = vault.path().join("doc.pdf");
    tokio::fs::write(&pdf_path, &pdf_bytes).await.unwrap();
    let rel = Path::new("doc.pdf");
    let data_dir = vault.path().join(".local-index");
    tokio::fs::create_dir_all(&data_dir).await.unwrap();

    let client = AnthropicAssetClient::new_for_test("test-key", server.uri());
    let cf = ingest_asset_path(
        vault.path(),
        rel,
        &data_dir,
        pdf_bytes.len() * 2,
        30,
        None,
        Some(&client),
    )
    .await
    .expect("ingest textfirst pdf with embedded image");

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(
        reqs.len(),
        1,
        "expected exactly one vision call (one embedded image); got {}",
        reqs.len()
    );

    let joined: String = cf
        .chunks
        .iter()
        .map(|c| c.body.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("> **[Image: doc_page_1_image_1.png]** EMBEDDED_IMG_DESC"),
        "expected embedded image blockquote; got: {joined}"
    );
}
