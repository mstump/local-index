use std::path::{Path, PathBuf};
use tracing::{debug, info, trace};
use walkdir::WalkDir;

/// Recursively discover all .md files in a directory tree, skipping hidden directories.
pub fn discover_markdown_files(vault_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in WalkDir::new(vault_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
    {
        match entry {
            Ok(entry) => {
                if entry.file_type().is_file() {
                    if entry.path().extension().is_some_and(|ext| ext == "md") {
                        debug!(path = %entry.path().display(), "discovered markdown file");
                        files.push(entry.into_path());
                    } else {
                        trace!(
                            path = %entry.path().display(),
                            "skipping non-markdown file"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "error walking directory");
            }
        }
    }

    info!(count = files.len(), "markdown file discovery complete");
    files
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    // Don't filter the root directory itself (depth 0)
    entry.depth() > 0
        && entry
            .file_name()
            .to_str()
            .is_some_and(|s| s.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_discover_md_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("note1.md"), "# Note 1").unwrap();
        fs::write(dir.path().join("note2.md"), "# Note 2").unwrap();
        fs::write(dir.path().join("readme.txt"), "not markdown").unwrap();
        fs::write(dir.path().join("data.json"), "{}").unwrap();
        fs::write(dir.path().join("note3.md"), "# Note 3").unwrap();

        let files = discover_markdown_files(dir.path());
        assert_eq!(files.len(), 3, "should find exactly 3 .md files");
        for f in &files {
            assert_eq!(f.extension().unwrap(), "md");
        }
    }

    #[test]
    fn test_skip_hidden_dirs() {
        let dir = tempdir().unwrap();
        let hidden = dir.path().join(".hidden");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(hidden.join("secret.md"), "# Secret").unwrap();
        fs::write(dir.path().join("visible.md"), "# Visible").unwrap();

        let files = discover_markdown_files(dir.path());
        assert_eq!(files.len(), 1, "should skip .hidden directory");
        assert!(files[0].ends_with("visible.md"));
    }

    #[test]
    fn test_empty_dir() {
        let dir = tempdir().unwrap();
        let files = discover_markdown_files(dir.path());
        assert!(files.is_empty(), "empty dir should return no files");
    }

    #[test]
    fn test_nested_dirs() {
        let dir = tempdir().unwrap();
        let deep = dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("deep.md"), "# Deep").unwrap();
        fs::write(dir.path().join("top.md"), "# Top").unwrap();
        fs::write(dir.path().join("a").join("mid.md"), "# Mid").unwrap();

        let files = discover_markdown_files(dir.path());
        assert_eq!(files.len(), 3, "should find .md files at all depth levels");
    }
}
