//! Ephemeral on-disk cache paths under the configured data directory.

use std::path::{Path, PathBuf};

/// Placeholder until Task 4.
pub fn cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("asset-cache")
}

/// Placeholder until Task 4.
pub fn cache_path_for_hash(data_dir: &Path, sha256_hex: &str) -> PathBuf {
    cache_dir(data_dir).join(sha256_hex)
}

/// Placeholder until Task 4.
pub fn ensure_cache_parent(_path: &Path) -> std::io::Result<()> {
    Ok(())
}
