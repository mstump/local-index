use crate::error::LocalIndexError;

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
}
