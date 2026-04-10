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
}
