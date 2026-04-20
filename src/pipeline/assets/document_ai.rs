//! Google Cloud Document AI `:process` client for scanned PDF pages (`PRE-07`, `PRE-08`).
//!
//! Transport is HTTPS to **`documentai.googleapis.com`** only — no configurable base URL in production.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::credentials::{validate_google_document_ai_config, ServiceAccountKey};
use crate::error::LocalIndexError;

/// Hard-coded API hostname pattern — location prefix selects regional endpoint (`us`, `eu`, …).
const DOCUMENT_AI_HOST_SUFFIX: &str = "documentai.googleapis.com";

#[derive(Clone)]
enum AuthMode {
    ServiceAccount(ServiceAccountKey),
    /// Integration tests (wiremock): skip JWT exchange.
    FixedBearer(String),
}

/// REST client for `processors.process` on raster images (`image/png`).
#[derive(Clone)]
pub struct DocumentAiClient {
    project_id: String,
    location: String,
    processor_id: String,
    http: reqwest::Client,
    auth: AuthMode,
    api_base_url: String,
    token_cache: Arc<Mutex<Option<(String, Instant)>>>,
}

impl DocumentAiClient {
    /// Build from env after [`validate_google_document_ai_config`] succeeds internally.
    pub fn new_from_env() -> Result<Self, LocalIndexError> {
        validate_google_document_ai_config()?;
        let project_id = std::env::var("GOOGLE_CLOUD_PROJECT").map_err(|_| {
            LocalIndexError::Credential(
                "GOOGLE_CLOUD_PROJECT missing after validation".to_string(),
            )
        })?;
        let location = std::env::var("GOOGLE_DOCUMENT_AI_LOCATION").map_err(|_| {
            LocalIndexError::Credential(
                "GOOGLE_DOCUMENT_AI_LOCATION missing after validation".to_string(),
            )
        })?;
        let processor_id = std::env::var("GOOGLE_DOCUMENT_AI_PROCESSOR_ID").map_err(|_| {
            LocalIndexError::Credential(
                "GOOGLE_DOCUMENT_AI_PROCESSOR_ID missing after validation".to_string(),
            )
        })?;
        let path = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").map_err(|_| {
            LocalIndexError::Credential(
                "GOOGLE_APPLICATION_CREDENTIALS missing after validation".to_string(),
            )
        })?;
        let sa_json = std::fs::read_to_string(&path).map_err(|e| {
            LocalIndexError::Credential(format!(
                "Failed to read service account file {path}: {e}"
            ))
        })?;
        let sa: ServiceAccountKey = serde_json::from_str(&sa_json).map_err(|e| {
            LocalIndexError::Credential(format!(
                "Invalid service account JSON at {path}: {e}"
            ))
        })?;
        let api_base_url = format!("https://{location}-{DOCUMENT_AI_HOST_SUFFIX}");
        Ok(Self {
            project_id,
            location,
            processor_id,
            http: reqwest::Client::new(),
            auth: AuthMode::ServiceAccount(sa),
            api_base_url,
            token_cache: Arc::new(Mutex::new(None)),
        })
    }

    /// Wiremock / tests: fixed bearer, custom API base (e.g. mock server root + path prefix if needed).
    pub fn new_for_test(
        project_id: impl Into<String>,
        location: impl Into<String>,
        processor_id: impl Into<String>,
        api_base_url: impl Into<String>,
        bearer: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            location: location.into(),
            processor_id: processor_id.into(),
            http: reqwest::Client::new(),
            auth: AuthMode::FixedBearer(bearer.into()),
            api_base_url: api_base_url.into(),
            token_cache: Arc::new(Mutex::new(None)),
        }
    }

    async fn access_token(&self) -> Result<String, LocalIndexError> {
        match &self.auth {
            AuthMode::FixedBearer(s) => Ok(s.clone()),
            AuthMode::ServiceAccount(sa) => {
                let mut guard = self.token_cache.lock().await;
                let now = Instant::now();
                if let Some((tok, at)) = guard.as_ref() {
                    if now.saturating_duration_since(*at) < Duration::from_secs(50 * 60) {
                        return Ok(tok.clone());
                    }
                }
                let tok = fetch_oauth_token(sa, &self.http).await?;
                *guard = Some((tok.clone(), now));
                Ok(tok)
            }
        }
    }

    /// One `:process` call for a single PNG page.
    pub async fn process_png_page(&self, png_bytes: &[u8]) -> Result<String, LocalIndexError> {
        let url = format!(
            "{}/v1/projects/{}/locations/{}/processors/{}:process",
            self.api_base_url.trim_end_matches('/'),
            self.project_id,
            self.location,
            self.processor_id
        );
        let body = json!({
            "rawDocument": {
                "content": B64.encode(png_bytes),
                "mimeType": "image/png"
            }
        });

        for attempt in 0u32..2 {
            let token = self.access_token().await?;
            let resp = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| LocalIndexError::AssetVision(format!("Document AI HTTP error: {e}")))?;

            let status = resp.status();
            if status.as_u16() == 429 && attempt == 0 {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                let prefix: String = text.chars().take(400).collect();
                return Err(LocalIndexError::AssetVision(format!(
                    "Document AI error {status}: {prefix}"
                )));
            }

            let parsed: ProcessEnvelope = resp
                .json()
                .await
                .map_err(|e| LocalIndexError::AssetVision(format!("Document AI JSON error: {e}")))?;

            let text = parsed
                .document
                .and_then(|d| d.text)
                .unwrap_or_default()
                .trim()
                .to_string();

            if text.is_empty() {
                return Err(LocalIndexError::AssetVision(
                    "Document AI returned empty document.text (OCR)".to_string(),
                ));
            }

            return Ok(text);
        }

        Err(LocalIndexError::AssetVision(
            "Document AI: rate limited after retry".to_string(),
        ))
    }
}

#[derive(Debug, Deserialize)]
struct ProcessEnvelope {
    document: Option<DocumentBody>,
}

#[derive(Debug, Deserialize)]
struct DocumentBody {
    text: Option<String>,
}

#[derive(Debug, Serialize)]
struct JwtClaims {
    iss: String,
    sub: String,
    scope: String,
    aud: String,
    exp: i64,
    iat: i64,
}

async fn fetch_oauth_token(
    sa: &ServiceAccountKey,
    http: &reqwest::Client,
) -> Result<String, LocalIndexError> {
    let now = Utc::now().timestamp();
    let exp = now + 3600;
    let email = sa.client_email.clone();
    let claims = JwtClaims {
        iss: email.clone(),
        sub: email,
        scope: "https://www.googleapis.com/auth/cloud-platform".to_string(),
        aud: sa.token_uri.clone(),
        exp,
        iat: now,
    };
    let key = EncodingKey::from_rsa_pem(sa.private_key.as_bytes()).map_err(|e| {
        LocalIndexError::Credential(format!("Invalid service account private key PEM: {e}"))
    })?;
    let jwt = encode(&Header::new(Algorithm::RS256), &claims, &key).map_err(|e| {
        LocalIndexError::Credential(format!("JWT encode error: {e}"))
    })?;

    let token_resp = http
        .post(&sa.token_uri)
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:jwt-bearer",
            ),
            ("assertion", jwt.as_str()),
        ])
        .send()
        .await
        .map_err(|e| LocalIndexError::Credential(format!("OAuth token HTTP error: {e}")))?;

    if !token_resp.status().is_success() {
        let t = token_resp.text().await.unwrap_or_default();
        return Err(LocalIndexError::Credential(format!(
            "OAuth token exchange failed: {}",
            t.chars().take(300).collect::<String>()
        )));
    }

    let parsed: TokenResponse = token_resp
        .json()
        .await
        .map_err(|e| LocalIndexError::Credential(format!("OAuth token JSON error: {e}")))?;

    parsed.access_token.ok_or_else(|| {
        LocalIndexError::Credential(
            "OAuth token response missing access_token".to_string(),
        )
    })
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
}
