//! End-to-end `index` with assets: wiremock Voyage + Anthropic (no live API keys).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use local_index::pipeline::store::ChunkStore;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// Minimal 1×1 PNG (transparent).
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

fn binary_path() -> PathBuf {
    // Prefer the binary built with this test run (not a stale `target/debug/local-index`).
    PathBuf::from(env!("CARGO_BIN_EXE_local-index"))
}

fn git_init(repo: &std::path::Path) {
    let status = Command::new("git")
        .args(["init"])
        .current_dir(repo)
        .status()
        .expect("git init");
    assert!(status.success(), "git init failed");
}

fn voyage_dynamic_response(req: &Request) -> ResponseTemplate {
    let body: Value = serde_json::from_slice(&req.body).unwrap_or(json!({}));
    let count = body["input"]
        .as_array()
        .filter(|a| !a.is_empty())
        .map(|a| a.len())
        .unwrap_or(1);
    let data: Vec<Value> = (0..count)
        .map(|i| {
            json!({
                "embedding": vec![0.02f32; 1024],
                "index": i,
            })
        })
        .collect();
    ResponseTemplate::new(200).set_body_json(json!({
        "data": data,
        "model": "voyage-3.5",
        "usage": { "total_tokens": count as u64 * 5u64 }
    }))
}

#[tokio::test]
async fn index_png_stores_chunks_with_asset_file_path() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(|req: &Request| voyage_dynamic_response(req))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [{"type": "text", "text": "A tiny square image."}],
            "id": "msg_assets_int",
            "model": "claude",
            "role": "assistant",
            "stop_reason": "end_turn",
            "type": "message",
            "usage": {"input_tokens": 1, "output_tokens": 1}
        })))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    git_init(&vault);
    fs::write(vault.join("note.md"), "# Hello\n\nBody text for embedding.\n").unwrap();
    fs::write(vault.join("test.png"), PNG_1X1).unwrap();

    let data_dir = dir.path().join("idx-data");
    let mock_base = server.uri();

    let path_var = std::env::var("PATH").unwrap_or_default();
    let output = Command::new(binary_path())
        .args([
            "--log-level",
            "warn",
            "--data-dir",
            data_dir.to_str().unwrap(),
            "index",
            vault.to_str().unwrap(),
        ])
        .current_dir(&vault)
        .env_clear()
        .env("PATH", &path_var)
        .env("VOYAGE_API_KEY", "test-voyage-key")
        .env("ANTHROPIC_API_KEY", "test-anthropic-key")
        .env("LOCAL_INDEX_VOYAGE_BASE_URL", mock_base.as_str())
        .env("LOCAL_INDEX_ANTHROPIC_BASE_URL", mock_base.as_str())
        .output()
        .expect("spawn local-index index");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "index should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let summary: Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("stdout JSON: {e}, raw={stdout:?}"));
    assert_eq!(
        summary["errors"].as_u64().unwrap_or(0),
        0,
        "index errors, stderr={stderr}, summary={summary}"
    );
    assert!(
        summary["assets_indexed"].as_u64().unwrap_or(0) >= 1,
        "expected assets_indexed >= 1, got {summary} stderr={stderr}"
    );

    let paths = ChunkStore::open(data_dir.to_str().unwrap())
        .await
        .expect("open store")
        .get_all_file_paths()
        .await
        .expect("list paths");

    let has_png = paths.iter().any(|p| {
        let norm = p.replace('\\', "/");
        norm.ends_with("test.png")
    });
    assert!(
        has_png,
        "expected a chunk with file_path ending in test.png, got {paths:?}"
    );
}
