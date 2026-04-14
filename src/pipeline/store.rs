use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use arrow_array::{
    Array, FixedSizeListArray, RecordBatch, RecordBatchIterator, StringArray, UInt32Array,
    types::Float32Type,
};
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use sha2::{Digest, Sha256};

use crate::error::LocalIndexError;
use crate::types::Chunk;

pub const TABLE_NAME: &str = "chunks";
pub const EMBEDDING_DIM: i32 = 1024;

/// Escape a string for safe interpolation into a LanceDB SQL filter expression.
/// Single quotes are doubled (SQL standard escaping) to prevent injection via
/// filenames that contain apostrophes (e.g. `it's a note.md`).
fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

/// Compute a deterministic SHA-256 content hash over body + heading_breadcrumb + frontmatter.
/// Used for incremental re-indexing: unchanged content produces the same hash,
/// so we can skip re-embedding.
pub fn compute_content_hash(chunk: &Chunk) -> String {
    let mut hasher = Sha256::new();
    hasher.update(chunk.body.as_bytes());
    hasher.update(chunk.heading_breadcrumb.as_bytes());
    let fm_json = serde_json::to_string(&chunk.frontmatter).unwrap_or_default();
    hasher.update(fm_json.as_bytes());
    let result = hasher.finalize();
    result.iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        write!(acc, "{:02x}", b).unwrap();
        acc
    })
}

/// Build the Arrow schema for the chunks table.
/// 10 columns: chunk_id, file_path, heading_breadcrumb, body, line_start, line_end,
/// frontmatter_json, content_hash, embedding_model, vector.
pub fn chunks_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new("file_path", DataType::Utf8, false),
        Field::new("heading_breadcrumb", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
        Field::new("line_start", DataType::UInt32, false),
        Field::new("line_end", DataType::UInt32, false),
        Field::new("frontmatter_json", DataType::Utf8, true),
        Field::new("content_hash", DataType::Utf8, false),
        Field::new("embedding_model", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIM,
            ),
            false,
        ),
    ]))
}

/// LanceDB-backed store for embedded chunks.
pub struct ChunkStore {
    #[allow(dead_code)]
    db: lancedb::Connection,
    table: lancedb::Table,
}

impl ChunkStore {
    /// Open an existing chunks table or create a new one if the database is empty.
    pub async fn open(db_path: &str) -> Result<Self, LocalIndexError> {
        let db = lancedb::connect(db_path)
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        let table_names = db
            .table_names()
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        let table = if table_names.contains(&TABLE_NAME.to_string()) {
            db.open_table(TABLE_NAME)
                .execute()
                .await
                .map_err(|e| LocalIndexError::Database(e.to_string()))?
        } else {
            db.create_empty_table(TABLE_NAME, chunks_schema())
                .execute()
                .await
                .map_err(|e| LocalIndexError::Database(e.to_string()))?
        };

        Ok(ChunkStore { db, table })
    }

    /// Store chunks with their embeddings and content hashes into LanceDB.
    ///
    /// - `chunks`: the Chunk structs to store
    /// - `embeddings`: parallel array of embedding vectors (one per chunk)
    /// - `hashes`: parallel array of content hashes (one per chunk)
    /// - `model_id`: the embedding model identifier
    pub async fn store_chunks(
        &self,
        chunks: &[Chunk],
        embeddings: &[Vec<f32>],
        hashes: &[String],
        model_id: &str,
    ) -> Result<(), LocalIndexError> {
        if chunks.is_empty() {
            return Ok(());
        }

        let n = chunks.len();

        // Build Arrow arrays for each column
        let chunk_ids: Vec<String> = (0..n).map(|_| uuid::Uuid::new_v4().to_string()).collect();
        let chunk_id_array = Arc::new(StringArray::from(chunk_ids)) as Arc<dyn arrow_array::Array>;

        let file_paths: Vec<String> = chunks
            .iter()
            .map(|c| c.file_path.to_string_lossy().to_string())
            .collect();
        let file_path_array =
            Arc::new(StringArray::from(file_paths)) as Arc<dyn arrow_array::Array>;

        let breadcrumbs: Vec<String> = chunks
            .iter()
            .map(|c| c.heading_breadcrumb.clone())
            .collect();
        let breadcrumb_array =
            Arc::new(StringArray::from(breadcrumbs)) as Arc<dyn arrow_array::Array>;

        let bodies: Vec<String> = chunks.iter().map(|c| c.body.clone()).collect();
        let body_array = Arc::new(StringArray::from(bodies)) as Arc<dyn arrow_array::Array>;

        let line_starts: Vec<u32> = chunks.iter().map(|c| c.line_start as u32).collect();
        let line_start_array =
            Arc::new(UInt32Array::from(line_starts)) as Arc<dyn arrow_array::Array>;

        let line_ends: Vec<u32> = chunks.iter().map(|c| c.line_end as u32).collect();
        let line_end_array = Arc::new(UInt32Array::from(line_ends)) as Arc<dyn arrow_array::Array>;

        let frontmatter_jsons: Vec<Option<String>> = chunks
            .iter()
            .map(|c| serde_json::to_string(&c.frontmatter).ok())
            .collect();
        let frontmatter_array =
            Arc::new(StringArray::from(frontmatter_jsons)) as Arc<dyn arrow_array::Array>;

        let hash_array =
            Arc::new(StringArray::from(hashes.to_vec())) as Arc<dyn arrow_array::Array>;

        let models: Vec<String> = vec![model_id.to_string(); n];
        let model_array = Arc::new(StringArray::from(models)) as Arc<dyn arrow_array::Array>;

        // Build the FixedSizeList vector column
        let embedding_values: Vec<Option<Vec<Option<f32>>>> = embeddings
            .iter()
            .map(|emb| Some(emb.iter().map(|&v| Some(v)).collect()))
            .collect();
        let vector_array = Arc::new(
            FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                embedding_values,
                EMBEDDING_DIM,
            ),
        ) as Arc<dyn arrow_array::Array>;

        let batch = RecordBatch::try_new(
            chunks_schema(),
            vec![
                chunk_id_array,
                file_path_array,
                breadcrumb_array,
                body_array,
                line_start_array,
                line_end_array,
                frontmatter_array,
                hash_array,
                model_array,
                vector_array,
            ],
        )
        .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        let schema = chunks_schema();
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        self.table
            .add(reader)
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get content hashes for all chunks belonging to a specific file.
    pub async fn get_hashes_for_file(
        &self,
        file_path: &str,
    ) -> Result<Vec<String>, LocalIndexError> {
        let batches: Vec<RecordBatch> = self
            .table
            .query()
            .only_if(format!("file_path = '{}'", escape_sql_string(file_path)))
            .select(lancedb::query::Select::Columns(vec![
                "content_hash".to_string(),
            ]))
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        let mut hashes = Vec::new();
        for batch in &batches {
            let col = batch
                .column_by_name("content_hash")
                .expect("content_hash column must exist");
            let string_array = col
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("content_hash must be StringArray");
            for i in 0..string_array.len() {
                if !string_array.is_null(i) {
                    hashes.push(string_array.value(i).to_string());
                }
            }
        }

        Ok(hashes)
    }

    /// Delete all chunks for a given file path.
    pub async fn delete_chunks_for_file(&self, file_path: &str) -> Result<(), LocalIndexError> {
        self.table
            .delete(&format!("file_path = '{}'", escape_sql_string(file_path)))
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;
        Ok(())
    }

    /// Delete chunks for every stored `file_path` that is not among the vault-relative paths
    /// implied by `discovered_md_paths` (absolute paths under `vault_path`).
    ///
    /// Call after indexing so files removed from disk disappear from search even when
    /// the daemon is not running. When `discovered_md_paths` is empty, every indexed file
    /// is treated as absent and removed.
    pub async fn prune_absent_markdown_files(
        &self,
        vault_path: &Path,
        discovered_md_paths: &[std::path::PathBuf],
    ) -> Result<usize, LocalIndexError> {
        let present: HashSet<String> = discovered_md_paths
            .iter()
            .filter_map(|abs| {
                abs.strip_prefix(vault_path)
                    .ok()
                    .map(|rel| rel.to_string_lossy().to_string())
            })
            .collect();

        let stored = self.get_all_file_paths().await?;
        let mut pruned_files = 0usize;
        for path in stored {
            if !present.contains(&path) {
                self.delete_chunks_for_file(&path).await?;
                pruned_files += 1;
            }
        }
        Ok(pruned_files)
    }

    /// Check if the stored embedding model matches the configured model.
    ///
    /// Returns:
    /// - `Ok(false)` if no data exists (fresh DB) or model matches
    /// - `Ok(true)` if model differs and force_reindex is true (caller should clear)
    /// - `Err(Database)` if model differs and force_reindex is false
    pub async fn check_model_consistency(
        &self,
        model_id: &str,
        force_reindex: bool,
    ) -> Result<bool, LocalIndexError> {
        let batches: Vec<RecordBatch> = self
            .table
            .query()
            .select(lancedb::query::Select::Columns(vec![
                "embedding_model".to_string(),
            ]))
            .limit(1)
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        // No rows means fresh DB
        if batches.is_empty() || batches.iter().all(|b| b.num_rows() == 0) {
            return Ok(false);
        }

        let first_batch = &batches[0];
        let col = first_batch
            .column_by_name("embedding_model")
            .expect("embedding_model column must exist");
        let string_array = col
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("embedding_model must be StringArray");
        let stored_model = string_array.value(0);

        if stored_model == model_id {
            return Ok(false);
        }

        if force_reindex {
            return Ok(true);
        }

        Err(LocalIndexError::Database(format!(
            "Embedding model mismatch: database contains '{}' but configured model is '{}'. \
             Run with --force-reindex to re-embed all chunks.",
            stored_model, model_id
        )))
    }

    /// Count total rows (chunks) in the table.
    pub async fn count_total_chunks(&self) -> Result<usize, LocalIndexError> {
        self.table
            .count_rows(None)
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))
    }

    /// Count distinct file paths stored in the table.
    pub async fn count_distinct_files(&self) -> Result<usize, LocalIndexError> {
        let paths = self.get_all_file_paths().await?;
        Ok(paths.len())
    }

    /// Get all distinct file paths stored in the table.
    pub async fn get_all_file_paths(&self) -> Result<Vec<String>, LocalIndexError> {
        let batches: Vec<RecordBatch> = self
            .table
            .query()
            .select(lancedb::query::Select::Columns(vec![
                "file_path".to_string(),
            ]))
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        let mut unique_paths = HashSet::new();
        for batch in &batches {
            let col = batch
                .column_by_name("file_path")
                .expect("file_path column must exist");
            let string_array = col
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("file_path must be StringArray");
            for i in 0..string_array.len() {
                if !string_array.is_null(i) {
                    unique_paths.insert(string_array.value(i).to_string());
                }
            }
        }

        Ok(unique_paths.into_iter().collect())
    }

    /// Returns a sorted list of (file_path, chunk_count) for all files in the index.
    pub async fn count_chunks_per_file(&self) -> Result<Vec<(String, usize)>, LocalIndexError> {
        let batches: Vec<RecordBatch> = self
            .table
            .query()
            .select(lancedb::query::Select::Columns(vec![
                "file_path".to_string(),
            ]))
            .execute()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;

        let mut counts: HashMap<String, usize> = HashMap::new();
        for batch in &batches {
            let col = batch
                .column_by_name("file_path")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            if let Some(arr) = col {
                for i in 0..arr.len() {
                    if !arr.is_null(i) {
                        *counts.entry(arr.value(i).to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut result: Vec<(String, usize)> = counts.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    /// Expose the underlying LanceDB table for search queries.
    pub fn table(&self) -> &lancedb::Table {
        &self.table
    }

    /// Clear all rows from the chunks table (for force-reindex).
    pub async fn clear_all(&self) -> Result<(), LocalIndexError> {
        self.table
            .delete("true")
            .await
            .map_err(|e| LocalIndexError::Database(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Frontmatter;
    use std::path::PathBuf;

    fn make_test_chunk(file_path: &str, body: &str, breadcrumb: &str) -> Chunk {
        Chunk {
            file_path: PathBuf::from(file_path),
            heading_breadcrumb: breadcrumb.to_string(),
            heading_level: 1,
            body: body.to_string(),
            line_start: 1,
            line_end: 10,
            frontmatter: Frontmatter::default(),
        }
    }

    fn make_test_embedding() -> Vec<f32> {
        vec![0.1_f32; EMBEDDING_DIM as usize]
    }

    #[tokio::test]
    async fn test_open_creates_table() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await;
        assert!(store.is_ok(), "open should succeed on empty dir");
    }

    #[tokio::test]
    async fn test_open_existing_table() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();

        // First open creates the table
        let _store1 = ChunkStore::open(db_path).await.unwrap();
        drop(_store1);

        // Second open should re-open existing table
        let store2 = ChunkStore::open(db_path).await;
        assert!(store2.is_ok(), "open should succeed on existing table");
    }

    #[tokio::test]
    async fn test_store_and_retrieve_hashes() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let chunks = vec![
            make_test_chunk("notes/test.md", "body one", "# Heading"),
            make_test_chunk("notes/test.md", "body two", "## Sub"),
        ];
        let embeddings = vec![make_test_embedding(), make_test_embedding()];
        let hashes = chunks
            .iter()
            .map(|c| compute_content_hash(c))
            .collect::<Vec<_>>();

        store
            .store_chunks(&chunks, &embeddings, &hashes, "voyage-3.5")
            .await
            .unwrap();

        let retrieved = store.get_hashes_for_file("notes/test.md").await.unwrap();
        assert_eq!(retrieved.len(), 2, "should retrieve 2 hashes");
    }

    #[tokio::test]
    async fn test_get_hashes_unknown_file() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let retrieved = store.get_hashes_for_file("nonexistent.md").await.unwrap();
        assert!(
            retrieved.is_empty(),
            "should return empty vec for unknown file"
        );
    }

    #[tokio::test]
    async fn test_delete_chunks_for_file() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        // Store chunks for two different files
        let chunks_a = vec![make_test_chunk("file_a.md", "body a", "# A")];
        let chunks_b = vec![make_test_chunk("file_b.md", "body b", "# B")];
        let emb = vec![make_test_embedding()];

        let hashes_a = chunks_a
            .iter()
            .map(|c| compute_content_hash(c))
            .collect::<Vec<_>>();
        let hashes_b = chunks_b
            .iter()
            .map(|c| compute_content_hash(c))
            .collect::<Vec<_>>();

        store
            .store_chunks(&chunks_a, &emb, &hashes_a, "voyage-3.5")
            .await
            .unwrap();
        store
            .store_chunks(&chunks_b, &emb, &hashes_b, "voyage-3.5")
            .await
            .unwrap();

        // Delete file_a chunks
        store.delete_chunks_for_file("file_a.md").await.unwrap();

        // file_a should be gone
        let hashes_a_result = store.get_hashes_for_file("file_a.md").await.unwrap();
        assert!(
            hashes_a_result.is_empty(),
            "file_a chunks should be deleted"
        );

        // file_b should still exist
        let hashes_b_result = store.get_hashes_for_file("file_b.md").await.unwrap();
        assert_eq!(hashes_b_result.len(), 1, "file_b chunks should still exist");
    }

    #[tokio::test]
    async fn prune_absent_markdown_files_removes_stale_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();
        let db_path = tmp.path().join("db").to_str().unwrap().to_string();
        let store = ChunkStore::open(&db_path).await.unwrap();

        let chunks_a = vec![make_test_chunk("gone.md", "a", "# A")];
        let chunks_b = vec![make_test_chunk("kept.md", "b", "# B")];
        let emb = vec![make_test_embedding()];
        let ha = chunks_a
            .iter()
            .map(|c| compute_content_hash(c))
            .collect::<Vec<_>>();
        let hb = chunks_b
            .iter()
            .map(|c| compute_content_hash(c))
            .collect::<Vec<_>>();
        store
            .store_chunks(&chunks_a, &emb, &ha, "voyage-3.5")
            .await
            .unwrap();
        store
            .store_chunks(&chunks_b, &emb, &hb, "voyage-3.5")
            .await
            .unwrap();

        let kept_abs = vault.join("kept.md");
        let n = store
            .prune_absent_markdown_files(&vault, &[kept_abs])
            .await
            .unwrap();
        assert_eq!(n, 1, "should prune exactly one absent file");

        assert!(
            store
                .get_hashes_for_file("gone.md")
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.get_hashes_for_file("kept.md").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_model_consistency_empty_db() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let result = store
            .check_model_consistency("voyage-3.5", false)
            .await
            .unwrap();
        assert_eq!(result, false, "empty DB should return Ok(false)");
    }

    #[tokio::test]
    async fn test_model_consistency_match() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let chunks = vec![make_test_chunk("test.md", "body", "# H")];
        let emb = vec![make_test_embedding()];
        let hashes = chunks
            .iter()
            .map(|c| compute_content_hash(c))
            .collect::<Vec<_>>();
        store
            .store_chunks(&chunks, &emb, &hashes, "voyage-3.5")
            .await
            .unwrap();

        let result = store
            .check_model_consistency("voyage-3.5", false)
            .await
            .unwrap();
        assert_eq!(result, false, "matching model should return Ok(false)");
    }

    #[tokio::test]
    async fn test_model_consistency_mismatch_no_force() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let chunks = vec![make_test_chunk("test.md", "body", "# H")];
        let emb = vec![make_test_embedding()];
        let hashes = chunks
            .iter()
            .map(|c| compute_content_hash(c))
            .collect::<Vec<_>>();
        store
            .store_chunks(&chunks, &emb, &hashes, "voyage-3.5")
            .await
            .unwrap();

        let result = store.check_model_consistency("voyage-4", false).await;
        assert!(result.is_err(), "mismatch without force should be Err");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Embedding model mismatch"),
            "error should mention model mismatch"
        );
        assert!(
            err_msg.contains("force-reindex"),
            "error should suggest --force-reindex"
        );
    }

    #[tokio::test]
    async fn test_model_consistency_mismatch_force() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let chunks = vec![make_test_chunk("test.md", "body", "# H")];
        let emb = vec![make_test_embedding()];
        let hashes = chunks
            .iter()
            .map(|c| compute_content_hash(c))
            .collect::<Vec<_>>();
        store
            .store_chunks(&chunks, &emb, &hashes, "voyage-3.5")
            .await
            .unwrap();

        let result = store
            .check_model_consistency("voyage-4", true)
            .await
            .unwrap();
        assert_eq!(result, true, "mismatch with force should return Ok(true)");
    }

    #[test]
    fn test_compute_content_hash_deterministic() {
        let chunk = make_test_chunk("test.md", "hello world", "# Title");
        let hash1 = compute_content_hash(&chunk);
        let hash2 = compute_content_hash(&chunk);
        assert_eq!(hash1, hash2, "same chunk should produce same hash");
        assert!(!hash1.is_empty(), "hash should not be empty");
    }

    #[test]
    fn test_compute_content_hash_body_change() {
        let chunk1 = make_test_chunk("test.md", "body one", "# Title");
        let chunk2 = make_test_chunk("test.md", "body two", "# Title");
        let hash1 = compute_content_hash(&chunk1);
        let hash2 = compute_content_hash(&chunk2);
        assert_ne!(hash1, hash2, "different body should produce different hash");
    }

    #[test]
    fn test_compute_content_hash_breadcrumb_change() {
        let chunk1 = make_test_chunk("test.md", "body", "# Title One");
        let chunk2 = make_test_chunk("test.md", "body", "# Title Two");
        let hash1 = compute_content_hash(&chunk1);
        let hash2 = compute_content_hash(&chunk2);
        assert_ne!(
            hash1, hash2,
            "different breadcrumb should produce different hash"
        );
    }

    #[tokio::test]
    async fn test_count_total_chunks_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let count = store.count_total_chunks().await.unwrap();
        assert_eq!(count, 0, "empty store should have 0 chunks");
    }

    #[tokio::test]
    async fn test_count_total_chunks_after_store() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let chunks = vec![
            make_test_chunk("a.md", "body1", "# H1"),
            make_test_chunk("a.md", "body2", "# H2"),
            make_test_chunk("b.md", "body3", "# H3"),
        ];
        let embs = vec![
            make_test_embedding(),
            make_test_embedding(),
            make_test_embedding(),
        ];
        let hashes: Vec<String> = chunks.iter().map(|c| compute_content_hash(c)).collect();
        store
            .store_chunks(&chunks, &embs, &hashes, "voyage-3.5")
            .await
            .unwrap();

        let count = store.count_total_chunks().await.unwrap();
        assert_eq!(count, 3, "should have 3 chunks");
    }

    #[tokio::test]
    async fn test_count_distinct_files_after_store() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let chunks = vec![
            make_test_chunk("a.md", "body1", "# H1"),
            make_test_chunk("a.md", "body2", "# H2"),
            make_test_chunk("b.md", "body3", "# H3"),
        ];
        let embs = vec![
            make_test_embedding(),
            make_test_embedding(),
            make_test_embedding(),
        ];
        let hashes: Vec<String> = chunks.iter().map(|c| compute_content_hash(c)).collect();
        store
            .store_chunks(&chunks, &embs, &hashes, "voyage-3.5")
            .await
            .unwrap();

        let count = store.count_distinct_files().await.unwrap();
        assert_eq!(count, 2, "should have 2 distinct files");
    }

    #[tokio::test]
    async fn test_count_distinct_files_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().to_str().unwrap();
        let store = ChunkStore::open(db_path).await.unwrap();

        let count = store.count_distinct_files().await.unwrap();
        assert_eq!(count, 0, "empty store should have 0 distinct files");
    }

    #[test]
    fn test_compute_content_hash_frontmatter_change() {
        let mut chunk1 = make_test_chunk("test.md", "body", "# Title");
        chunk1.frontmatter.tags = vec!["tag1".to_string()];

        let mut chunk2 = make_test_chunk("test.md", "body", "# Title");
        chunk2.frontmatter.tags = vec!["tag2".to_string()];

        let hash1 = compute_content_hash(&chunk1);
        let hash2 = compute_content_hash(&chunk2);
        assert_ne!(
            hash1, hash2,
            "different frontmatter should produce different hash"
        );
    }
}
