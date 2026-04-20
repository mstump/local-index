//! Ephemeral on-disk cache paths under the configured data directory.

use std::path::{Path, PathBuf};

/// Root directory for extracted / derived asset text under `data_dir`.
pub fn cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("asset-cache")
}

/// Sharded cache file path for a SHA-256 content hash (hex, 64 chars).
///
/// Layout: `asset-cache/ab/cd/{sha256}.txt` — two-byte shard prefix for directory fan-out.
pub fn cache_path_for_hash(data_dir: &Path, sha256_hex: &str) -> PathBuf {
    let shard = if sha256_hex.len() >= 4 {
        format!(
            "{}/{}",
            &sha256_hex[..2],
            &sha256_hex[2..4]
        )
    } else {
        "_/_".to_string()
    };
    cache_dir(data_dir).join(shard).join(format!("{sha256_hex}.txt"))
}

/// Ensure parent directories exist for a cache file path (sync I/O, matches store-style helpers).
pub fn ensure_cache_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cache_path_for_hash_is_stable() {
        let dir = tempdir().unwrap();
        let h = "deadbeef".repeat(8); // 64 hex chars
        let p1 = cache_path_for_hash(dir.path(), &h);
        let p2 = cache_path_for_hash(dir.path(), &h);
        assert_eq!(p1, p2);
        assert!(p1.to_string_lossy().contains("asset-cache"));
        assert!(p1.to_string_lossy().contains("de/ad"));
        assert!(p1.ends_with(format!("{h}.txt")));
    }

    #[tokio::test]
    async fn read_cache_if_present_returns_content_when_non_empty() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("hit.txt");
        tokio::fs::write(&p, b"hello\n").await.unwrap();
        let got = read_cache_if_present(&p).await;
        assert_eq!(got.as_deref(), Some("hello\n"));
    }

    #[tokio::test]
    async fn read_cache_if_present_returns_none_for_empty_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("empty.txt");
        tokio::fs::write(&p, b"").await.unwrap();
        let got = read_cache_if_present(&p).await;
        assert!(got.is_none(), "empty file should be treated as miss");
    }

    #[tokio::test]
    async fn read_cache_if_present_returns_none_for_whitespace_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("whitespace.txt");
        tokio::fs::write(&p, b"   \n").await.unwrap();
        let got = read_cache_if_present(&p).await;
        assert!(got.is_none(), "whitespace-only file should be treated as miss");
    }

    #[tokio::test]
    async fn read_cache_if_present_returns_none_silently_when_missing() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("does_not_exist.txt");
        let got = read_cache_if_present(&p).await;
        assert!(got.is_none(), "NotFound should be a silent miss");
    }

    #[tokio::test]
    async fn read_cache_if_present_returns_none_when_path_is_directory() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().join("not_a_file");
        std::fs::create_dir_all(&dir_path).unwrap();
        let got = read_cache_if_present(&dir_path).await;
        assert!(got.is_none(), "directory path should produce miss with WARN");
    }
}
