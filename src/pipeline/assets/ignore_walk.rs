//! Gitignore-aware asset discovery under a vault root.
//!
//! ## `exclude_globs` — [`ignore::overrides::OverrideBuilder`]
//! Each pattern is registered with a leading `!`, which [`OverrideBuilder::add`] treats as an
//! **exclusion** (see ignore crate docs: patterns starting with `!` exclude matching paths).

use std::path::{Path, PathBuf};

use ignore::overrides::{Override, OverrideBuilder};
use ignore::WalkBuilder;

use crate::error::LocalIndexError;

/// Build an [`Override`] matcher for operator `exclude_asset_globs` (same semantics as discovery).
///
/// Returns [`Override::empty`] when `exclude_globs` is empty.
pub(crate) fn build_asset_exclude_override(
    vault_root: &Path,
    exclude_globs: &[String],
) -> Result<Override, LocalIndexError> {
    if exclude_globs.is_empty() {
        return Ok(Override::empty());
    }
    let mut ob = OverrideBuilder::new(vault_root);
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
    Ok(ob.build()?)
}

/// True when `file_path` is excluded by operator globs (vault must be canonical for stable prefix checks).
pub(crate) fn is_asset_path_excluded_by_override(
    vault_canonical: &Path,
    file_path: &Path,
    exclude_override: &Override,
) -> bool {
    if exclude_override.is_empty() {
        return false;
    }
    let file_canon = match file_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let rel = match file_canon.strip_prefix(vault_canonical) {
        Ok(r) => r,
        Err(_) => return false,
    };
    exclude_override.matched(rel, false).is_ignore()
}

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
        builder.overrides(build_asset_exclude_override(&vault_root, exclude_globs)?);
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

    #[test]
    fn exclude_override_single_path_matches_walk() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        git_init(root);
        fs::create_dir_all(root.join("skipme")).unwrap();
        let skip_pdf = root.join("skipme").join("a.pdf");
        fs::write(&skip_pdf, b"%PDF").unwrap();
        let keep_pdf = root.join("keep.pdf");
        fs::write(&keep_pdf, b"%PDF").unwrap();

        let root = root.canonicalize().unwrap();
        let ov = build_asset_exclude_override(&root, &["**/skipme/**".to_string()]).unwrap();
        assert!(is_asset_path_excluded_by_override(&root, &skip_pdf, &ov));
        assert!(!is_asset_path_excluded_by_override(&root, &keep_pdf, &ov));
    }
}
