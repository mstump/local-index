use serde::Deserialize;

use crate::error::LocalIndexError;

/// Which backend performs OCR on **rasterized scanned PDF** pages (`LOCAL_INDEX_OCR_PROVIDER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "lower")]
pub enum OcrProvider {
    /// Anthropic Messages vision (`describe_raster_page`) — default.
    Anthropic,
    /// Google Cloud Document AI `:process` REST API.
    Google,
}

/// Service account JSON (`GOOGLE_APPLICATION_CREDENTIALS`) — used only for Google OCR.
#[derive(Debug, Deserialize, Clone)]
pub struct ServiceAccountKey {
    pub client_email: String,
    pub private_key: String,
    pub token_uri: String,
}

/// Resolve OCR provider from `--ocr-provider` when set, else `LOCAL_INDEX_OCR_PROVIDER`, else Anthropic.
pub fn resolve_ocr_provider(cli: Option<OcrProvider>) -> OcrProvider {
    if let Some(p) = cli {
        return p;
    }
    match std::env::var("LOCAL_INDEX_OCR_PROVIDER") {
        Ok(s) => parse_ocr_provider_env(&s).unwrap_or(OcrProvider::Anthropic),
        Err(_) => OcrProvider::Anthropic,
    }
}

fn parse_ocr_provider_env(s: &str) -> Option<OcrProvider> {
    match s.trim().to_ascii_lowercase().as_str() {
        "google" => Some(OcrProvider::Google),
        "anthropic" => Some(OcrProvider::Anthropic),
        _ => None,
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Validates Google Document AI env when operator selects [`OcrProvider::Google`].
pub fn validate_google_document_ai_config() -> Result<(), LocalIndexError> {
    let mut missing = Vec::new();
    if env_nonempty("GOOGLE_CLOUD_PROJECT").is_none() {
        missing.push("GOOGLE_CLOUD_PROJECT");
    }
    if env_nonempty("GOOGLE_DOCUMENT_AI_LOCATION").is_none() {
        missing.push("GOOGLE_DOCUMENT_AI_LOCATION");
    }
    if env_nonempty("GOOGLE_DOCUMENT_AI_PROCESSOR_ID").is_none() {
        missing.push("GOOGLE_DOCUMENT_AI_PROCESSOR_ID");
    }
    let cred_path = env_nonempty("GOOGLE_APPLICATION_CREDENTIALS");
    if cred_path.is_none() {
        missing.push("GOOGLE_APPLICATION_CREDENTIALS");
    }
    if !missing.is_empty() {
        return Err(LocalIndexError::Credential(format!(
            "Google Document AI OCR requires: {}. Create a processor in Google Cloud Console \
             (Document AI) and point GOOGLE_APPLICATION_CREDENTIALS at a service account JSON key.",
            missing.join(", ")
        )));
    }
    let path = cred_path.unwrap();
    let meta = std::fs::metadata(&path).map_err(|e| {
        LocalIndexError::Credential(format!(
            "GOOGLE_APPLICATION_CREDENTIALS path not readable ({path}): {e}"
        ))
    })?;
    if !meta.is_file() {
        return Err(LocalIndexError::Credential(format!(
            "GOOGLE_APPLICATION_CREDENTIALS must be a file path: {path}"
        )));
    }
    Ok(())
}

/// Resolve the Voyage AI API key from the environment.
///
/// Checks `VOYAGE_API_KEY` env var only (no ~/.claude/ fallback per D-04).
/// Returns a clear, actionable error message on missing credentials (per D-05).
pub fn resolve_voyage_key() -> Result<String, LocalIndexError> {
    std::env::var("VOYAGE_API_KEY").map_err(|_| {
        LocalIndexError::Credential(
            "VOYAGE_API_KEY environment variable not set. \
             Get your API key from https://dash.voyageai.com/ and set it: \
             export VOYAGE_API_KEY=your-key-here"
                .to_string(),
        )
    })
}

/// Resolve the Anthropic API key for PDF/image vision extraction (`PRE-14`).
///
/// Used when a scanned PDF or raster image must be described via the Messages API.
pub fn resolve_anthropic_key_for_assets() -> Result<String, LocalIndexError> {
    let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        LocalIndexError::Credential(
            "ANTHROPIC_API_KEY environment variable not set. \
             Required for scanned PDFs and images during asset indexing. \
             Get a key from https://console.anthropic.com/ and set: \
             export ANTHROPIC_API_KEY=your-key-here"
                .to_string(),
        )
    })?;
    if key.trim().is_empty() {
        return Err(LocalIndexError::Credential(
            "ANTHROPIC_API_KEY is empty. \
             Get a key from https://console.anthropic.com/ and set a non-empty value."
                .to_string(),
        ));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_resolve_voyage_key_set() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let key = "test-voyage-key-12345";
        unsafe { std::env::set_var("VOYAGE_API_KEY", key) };
        let result = resolve_voyage_key();
        unsafe { std::env::remove_var("VOYAGE_API_KEY") };
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), key);
    }

    #[test]
    fn test_resolve_voyage_key_unset() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe { std::env::remove_var("VOYAGE_API_KEY") };
        let result = resolve_voyage_key();
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("VOYAGE_API_KEY"),
            "error should mention VOYAGE_API_KEY: {}",
            msg
        );
        assert!(
            msg.contains("https://dash.voyageai.com/"),
            "error should contain actionable guidance: {}",
            msg
        );
    }

    #[test]
    fn test_resolve_anthropic_key_for_assets_set() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe { std::env::remove_var("VOYAGE_API_KEY") };
        let key = "test-anthropic-asset-key";
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", key) };
        let result = resolve_anthropic_key_for_assets();
        unsafe { std::env::remove_var("ANTHROPIC_API_KEY") };
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), key);
    }

    #[test]
    fn test_resolve_anthropic_key_for_assets_unset() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe { std::env::remove_var("ANTHROPIC_API_KEY") };
        let result = resolve_anthropic_key_for_assets();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("ANTHROPIC_API_KEY"), "msg={msg}");
        assert!(
            msg.contains("https://console.anthropic.com/"),
            "msg={msg}"
        );
    }

    #[test]
    fn google_ocr_validation_lists_missing_keys() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("LOCAL_INDEX_OCR_PROVIDER", "google");
            std::env::remove_var("GOOGLE_CLOUD_PROJECT");
            std::env::remove_var("GOOGLE_DOCUMENT_AI_LOCATION");
            std::env::remove_var("GOOGLE_DOCUMENT_AI_PROCESSOR_ID");
            std::env::remove_var("GOOGLE_APPLICATION_CREDENTIALS");
        }
        let err = validate_google_document_ai_config().unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("GOOGLE_CLOUD_PROJECT"),
            "expected project hint: {msg}"
        );
        assert!(
            msg.contains("GOOGLE_DOCUMENT_AI_PROCESSOR_ID"),
            "expected processor hint: {msg}"
        );
        unsafe {
            std::env::remove_var("LOCAL_INDEX_OCR_PROVIDER");
        }
    }
}
