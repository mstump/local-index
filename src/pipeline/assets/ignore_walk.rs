//! Gitignore-aware asset discovery under a vault root.

use std::path::{Path, PathBuf};

/// Placeholder until Task 2 implements discovery.
pub fn discover_asset_paths(
    _vault_root: &Path,
    _extensions: &[&str],
    _exclude_globs: &[String],
) -> Result<Vec<PathBuf>, crate::error::LocalIndexError> {
    Ok(Vec::new())
}
