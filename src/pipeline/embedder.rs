use crate::error::LocalIndexError;
use crate::types::EmbeddingResult;
use serde::{Deserialize, Serialize};

/// Trait for embedding providers. Designed so adding a new provider (e.g., Gemini, OpenAI)
/// requires only a new struct implementation — no changes to the pipeline.
pub trait Embedder: Send + Sync {
    /// Embed a batch of texts, returning vectors and usage metadata.
    fn embed(
        &self,
        texts: &[String],
    ) -> impl std::future::Future<Output = Result<EmbeddingResult, LocalIndexError>> + Send;

    /// Return the model identifier (e.g., "voyage-3.5").
    fn model_id(&self) -> &str;

    /// Return the embedding dimensionality.
    fn dimensions(&self) -> usize;
}

impl<E: Embedder> Embedder for std::sync::Arc<E> {
    fn embed(
        &self,
        texts: &[String],
    ) -> impl std::future::Future<Output = Result<EmbeddingResult, LocalIndexError>> + Send {
        (**self).embed(texts)
    }

    fn model_id(&self) -> &str {
        (**self).model_id()
    }

    fn dimensions(&self) -> usize {
        (**self).dimensions()
    }
}

// -- Voyage AI API types --

#[derive(Debug, Serialize)]
pub struct VoyageRequest {
    pub input: Vec<String>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    pub truncation: bool,
}

#[derive(Debug, Deserialize)]
pub struct VoyageResponse {
    pub data: Vec<VoyageEmbedding>,
    pub model: String,
    pub usage: VoyageUsage,
}

#[derive(Debug, Deserialize)]
pub struct VoyageEmbedding {
    pub embedding: Vec<f32>,
    pub index: usize,
}

#[derive(Debug, Deserialize)]
pub struct VoyageUsage {
    pub total_tokens: u64,
}

// -- VoyageEmbedder --

const MAX_RETRIES: u32 = 5;
const BASE_DELAY_MS: u64 = 500;
const MAX_DELAY_MS: u64 = 30_000;
const DEFAULT_BATCH_SIZE: usize = 50;

pub struct VoyageEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dim: usize,
    base_url: String,
    batch_size: usize,
}

impl VoyageEmbedder {
    /// Create a new VoyageEmbedder with default settings (voyage-3.5, 1024 dims).
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model: "voyage-3.5".to_string(),
            dim: 1024,
            base_url: std::env::var("LOCAL_INDEX_VOYAGE_BASE_URL")
                .unwrap_or_else(|_| "https://api.voyageai.com".to_string()),
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }

    /// Create a VoyageEmbedder with a custom base URL (for testing with wiremock).
    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model: "voyage-3.5".to_string(),
            dim: 1024,
            base_url,
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }

    /// Embed a single batch with retry logic for transient errors.
    async fn embed_batch_with_retry(
        &self,
        batch: Vec<String>,
    ) -> Result<EmbeddingResult, LocalIndexError> {
        let url = format!("{}/v1/embeddings", self.base_url);
        let request_body = VoyageRequest {
            input: batch,
            model: self.model.clone(),
            input_type: Some("document".to_string()),
            truncation: true,
        };

        let mut attempt = 0u32;

        loop {
            attempt += 1;

            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request_body)
                .send()
                .await
                .map_err(|e| LocalIndexError::Embedding(format!("HTTP request failed: {}", e)))?;

            let status = response.status();

            if status.is_success() {
                let body: VoyageResponse = response.json().await.map_err(|e| {
                    LocalIndexError::Embedding(format!("Failed to parse response: {}", e))
                })?;

                // Sort embeddings by index to maintain input order
                let mut embeddings = body.data;
                embeddings.sort_by_key(|e| e.index);

                return Ok(EmbeddingResult {
                    embeddings: embeddings.into_iter().map(|e| e.embedding).collect(),
                    model: body.model,
                    total_tokens: body.usage.total_tokens,
                });
            }

            // Auth errors: fail immediately, no retry
            if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                return Err(LocalIndexError::Embedding(
                    "Authentication failed: invalid VOYAGE_API_KEY".to_string(),
                ));
            }

            // Transient errors: retry with backoff
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
                if attempt >= MAX_RETRIES {
                    return Err(LocalIndexError::Embedding(format!(
                        "API request failed after {} retries: {}",
                        MAX_RETRIES, status
                    )));
                }

                let base_delay = BASE_DELAY_MS * 2u64.pow(attempt - 1);
                let jitter = rand::random::<u64>() % (base_delay / 2 + 1);
                let delay = (base_delay + jitter).min(MAX_DELAY_MS);

                tracing::warn!(
                    attempt = attempt,
                    max_retries = MAX_RETRIES,
                    delay_ms = delay,
                    status = %status,
                    "transient API error, retrying"
                );

                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                continue;
            }

            // Other errors: fail immediately
            let error_body = response.text().await.unwrap_or_default();
            return Err(LocalIndexError::Embedding(format!(
                "API request failed with status {}: {}",
                status, error_body
            )));
        }
    }
}

impl Embedder for VoyageEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<EmbeddingResult, LocalIndexError> {
        if texts.is_empty() {
            return Ok(EmbeddingResult {
                embeddings: vec![],
                model: self.model.clone(),
                total_tokens: 0,
            });
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());
        let mut total_tokens = 0u64;
        let mut model_name = self.model.clone();

        for batch in texts.chunks(self.batch_size) {
            let result = self.embed_batch_with_retry(batch.to_vec()).await?;
            all_embeddings.extend(result.embeddings);
            total_tokens += result.total_tokens;
            model_name = result.model;
        }

        Ok(EmbeddingResult {
            embeddings: all_embeddings,
            model: model_name,
            total_tokens,
        })
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn mock_voyage_response(count: usize, tokens: u64) -> serde_json::Value {
        let data: Vec<serde_json::Value> = (0..count)
            .map(|i| {
                serde_json::json!({
                    "embedding": vec![0.1f32; 1024],
                    "index": i,
                })
            })
            .collect();

        serde_json::json!({
            "data": data,
            "model": "voyage-3.5",
            "usage": { "total_tokens": tokens }
        })
    }

    #[tokio::test]
    async fn test_embed_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_voyage_response(2, 100)))
            .mount(&server)
            .await;

        let embedder = VoyageEmbedder::with_base_url("test-key".into(), server.uri());
        let result = embedder
            .embed(&["hello".to_string(), "world".to_string()])
            .await
            .unwrap();

        assert_eq!(result.embeddings.len(), 2);
        assert_eq!(result.embeddings[0].len(), 1024);
        assert_eq!(result.total_tokens, 100);
        assert_eq!(result.model, "voyage-3.5");
    }

    #[tokio::test]
    async fn test_embed_auth_failure() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1) // Should only be called once (no retry)
            .mount(&server)
            .await;

        let embedder = VoyageEmbedder::with_base_url("bad-key".into(), server.uri());
        let result = embedder.embed(&["hello".to_string()]).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Authentication failed"),
            "should contain auth error: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_embed_retry_on_429() {
        let server = MockServer::start().await;

        // First request returns 429, second returns 200
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_voyage_response(1, 50)))
            .mount(&server)
            .await;

        let embedder = VoyageEmbedder::with_base_url("test-key".into(), server.uri());
        let result = embedder.embed(&["hello".to_string()]).await;

        assert!(result.is_ok(), "should succeed after retry: {:?}", result);
        assert_eq!(result.unwrap().total_tokens, 50);
    }

    #[tokio::test]
    async fn test_embed_retry_exhausted() {
        let server = MockServer::start().await;

        // Always return 500
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let embedder = VoyageEmbedder::with_base_url("test-key".into(), server.uri());
        let result = embedder.embed(&["hello".to_string()]).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("retries"),
            "should mention retries: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_embed_batching() {
        let server = MockServer::start().await;

        // Mock that responds to any request with appropriate count
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                // This mock returns a generic response; we check request count
                mock_voyage_response(50, 500),
            ))
            .expect(3) // 120 texts / 50 per batch = 3 batches (50 + 50 + 20)
            .mount(&server)
            .await;

        let embedder = VoyageEmbedder::with_base_url("test-key".into(), server.uri());
        let texts: Vec<String> = (0..120).map(|i| format!("text {}", i)).collect();
        let result = embedder.embed(&texts).await;

        assert!(result.is_ok(), "batched embed should succeed: {:?}", result);
        // Note: mock returns 50 embeddings per batch, so we get 150 total
        // (in real usage, each batch response matches the batch size)
        let r = result.unwrap();
        assert_eq!(r.embeddings.len(), 150); // 3 batches * 50 each from mock
        assert_eq!(r.total_tokens, 1500); // 3 batches * 500 each from mock
    }

    #[test]
    fn test_model_id() {
        let embedder = VoyageEmbedder::new("test-key".into());
        assert_eq!(embedder.model_id(), "voyage-3.5");
    }

    #[test]
    fn test_dimensions() {
        let embedder = VoyageEmbedder::new("test-key".into());
        assert_eq!(embedder.dimensions(), 1024);
    }

    #[tokio::test]
    async fn test_embed_retry_success_on_second_attempt() {
        let server = MockServer::start().await;

        // First: 500, Second: 200
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_voyage_response(1, 25)))
            .mount(&server)
            .await;

        let embedder = VoyageEmbedder::with_base_url("test-key".into(), server.uri());
        let result = embedder.embed(&["hello".to_string()]).await;

        assert!(
            result.is_ok(),
            "should succeed on 2nd attempt: {:?}",
            result
        );
        assert_eq!(result.unwrap().total_tokens, 25);
    }
}
