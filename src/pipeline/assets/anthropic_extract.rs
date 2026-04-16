//! Anthropic Messages API for image / raster-page descriptions (`PRE-14`, `D-05` / `D-06`).

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::Deserialize;

use crate::credentials::resolve_anthropic_key_for_assets;
use crate::error::LocalIndexError;

const DEFAULT_MODEL: &str = "claude-3-5-haiku-20241022";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Fixed user prompt for vision requests (wiremock / contract tests match this string exactly).
pub const ASSET_VISION_PROMPT: &str =
    "Describe this image for search indexing: include visible text, charts, and diagrams.";

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

/// HTTP client for Anthropic vision calls used by the asset preprocessor.
#[derive(Debug, Clone)]
pub struct AnthropicAssetClient {
    api_key: String,
    model: String,
    http: reqwest::Client,
    base_url: String,
}

impl AnthropicAssetClient {
    /// Build a client aimed at a mock server (wiremock) without reading `ANTHROPIC_API_KEY`.
    pub fn new_for_test(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: DEFAULT_MODEL.to_string(),
            http: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Build from `ANTHROPIC_API_KEY` and optional `LOCAL_INDEX_ASSET_MODEL`.
    pub fn new_from_env() -> Result<Self, LocalIndexError> {
        let api_key = resolve_anthropic_key_for_assets()?;
        let model = std::env::var("LOCAL_INDEX_ASSET_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let base_url = std::env::var("LOCAL_INDEX_ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
        Ok(Self {
            api_key,
            model,
            http: reqwest::Client::new(),
            base_url,
        })
    }

    /// Redirect API calls (wiremock / integration tests).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Describe a raster image given raw bytes and an IANA media type (e.g. `image/png`).
    pub async fn describe_image(
        &self,
        media_type: &str,
        bytes: &[u8],
    ) -> Result<String, LocalIndexError> {
        let b64 = B64.encode(bytes);
        let url = format!(
            "{}/v1/messages",
            self.base_url.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": media_type,
                            "data": b64,
                        }
                    },
                    {
                        "type": "text",
                        "text": ASSET_VISION_PROMPT,
                    }
                ]
            }]
        });

        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LocalIndexError::AssetVision(format!("HTTP error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let prefix: String = text.chars().take(200).collect();
            return Err(LocalIndexError::AssetVision(format!(
                "Anthropic API error {status}: {prefix}"
            )));
        }

        let parsed: MessagesResponse = resp
            .json()
            .await
            .map_err(|e| LocalIndexError::AssetVision(format!("response JSON error: {e}")))?;

        let text = parsed
            .content
            .iter()
            .find(|b| b.block_type == "text")
            .and_then(|b| b.text.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if text.is_empty() {
            return Err(LocalIndexError::AssetVision(
                "Anthropic returned no text content".to_string(),
            ));
        }

        Ok(text)
    }

    /// Describe one PNG page (NeedsVision PDF path).
    pub async fn describe_raster_page(&self, png_bytes: &[u8]) -> Result<String, LocalIndexError> {
        self.describe_image("image/png", png_bytes).await
    }
}
