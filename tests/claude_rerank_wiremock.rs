use local_index::claude_rerank::AnthropicReranker;
use local_index::search::SearchResult;
use local_index::search::types::LineRange;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_result(label: &str) -> SearchResult {
    SearchResult {
        chunk_text: format!("body {}", label),
        file_path: format!("{}.md", label),
        heading_breadcrumb: "# H".to_string(),
        similarity_score: 0.5,
        semantic_score: None,
        fts_score: None,
        line_range: LineRange { start: 1, end: 2 },
        frontmatter: serde_json::json!({}),
        is_context: None,
        context_for_index: None,
    }
}

#[tokio::test]
async fn rerank_calls_anthropic_and_reorders() {
    let server = MockServer::start().await;
    let body = serde_json::json!({
        "id": "msg_test",
        "type": "message",
        "role": "assistant",
        "model": "claude-3-5-haiku-20241022",
        "stop_reason": "end_turn",
        "content": [
            {"type": "text", "text": "{\"indices\":[1,0]}"}
        ]
    });
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&server)
        .await;

    let reranker = AnthropicReranker::new("test-api-key".into()).with_base_url(server.uri());

    let a = sample_result("a");
    let b = sample_result("b");
    let out = reranker.rerank("find notes", vec![a, b]).await.unwrap();

    assert_eq!(out[0].file_path, "b.md");
    assert_eq!(out[1].file_path, "a.md");
    assert!(out[0].similarity_score >= out[1].similarity_score);
}
