//! Gitignore-aware asset discovery under a vault root.
//!
//! ## `exclude_globs` — [`ignore::overrides::OverrideBuilder`]
//! Each pattern is registered with a leading `!`, which [`OverrideBuilder::add`] treats as an
//! **exclusion** (see ignore crate docs: patterns starting with `!` exclude matching paths).

use std::path::{Path, PathBuf};

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;

use crate::error::LocalIndexError;

/// Discover asset files under `vault_root` with gitignore rules applied (`PRE-03`).
///
/// Returns paths **relative to `vault_root`** (as stored in the walk results after stripping the
/// canonical vault prefix), matching how operators reason about vault-relative paths.
pub fn discover_asset_paths(
    vault_root: &Path,
    extensions: &[&str],
    exclude_globs: &[String],
) -> Result<Vec<PathBuf>, LocalIndexError> {
    let vault_root = vault_root.canonicalize()?;
    let ext_ok = |p: &Path| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.iter().any(|want| want.eq_ignore_ascii_case(e)))
    };

    let mut builder = WalkBuilder::new(&vault_root);
    builder.git_ignore(true);
    builder.hidden(false);
    builder.follow_links(false);
    builder.standard_filters(true);
    builder.filter_entry(|entry| {
        if entry.depth() == 0 {
            return true;
        }
        entry
            .file_name()
            .to_str()
            .map(|name| !name.starts_with('.'))
            .unwrap_or(true)
    });

    if !exclude_globs.is_empty() {
        let mut ob = OverrideBuilder::new(&vault_root);
        for g in exclude_globs {
            let pattern = if g.starts_with('!') {
                g.clone()
            } else {
                format!("!{g}")
            };
            ob.add(&pattern).map_err(|e| {
                LocalIndexError::Config(format!("invalid exclude glob `{pattern}`: {e}"))
            })?;
        }
        builder.overrides(ob.build()?);
    }

    let mut out = Vec::new();
    for entry in builder.build() {
        let entry = entry?;
        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        if !ext_ok(path) {
            continue;
        }
        let rel = path.strip_prefix(&vault_root).map_err(|_| {
            LocalIndexError::Config(format!(
                "path {} is not under vault root {}",
                path.display(),
                vault_root.display()
            ))
        })?;
        out.push(rel.to_path_buf());
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    fn git_init(repo: &Path) {
        let status = Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");
    }

    #[test]
    fn gitignored_pdf_is_excluded() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        git_init(root);
        fs::write(root.join(".gitignore"), "secret.pdf\n").unwrap();
        fs::write(root.join("vis.pdf"), b"%PDF stub").unwrap();
        fs::write(root.join("secret.pdf"), b"%PDF stub").unwrap();

        let paths = discover_asset_paths(root, &["pdf"], &[]).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("vis.pdf"), "paths={paths:?}");
    }

    #[test]
    fn exclude_globs_filter_matches() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        git_init(root);
        fs::create_dir_all(root.join("skipme")).unwrap();
        fs::write(root.join("skipme").join("a.pdf"), b"%PDF").unwrap();
        fs::write(root.join("keep.pdf"), b"%PDF").unwrap();

        let paths = discover_asset_paths(root, &["pdf"], &["**/skipme/**".to_string()]).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("keep.pdf"), "paths={paths:?}");
    }
}
