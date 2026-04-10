use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Parsed YAML frontmatter from a markdown file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    /// Catch-all for unknown frontmatter fields
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yml::Value>,
}

/// A single chunk extracted from a markdown file
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// Vault-relative file path
    pub file_path: PathBuf,
    /// Heading hierarchy active at the start of this chunk (e.g., "# H1 > ## H2").
    /// Used for display and filtering. A chunk body may contain text from multiple headings.
    pub heading_breadcrumb: String,
    /// The heading level of this chunk's immediate heading (0 = pre-heading content)
    pub heading_level: u8,
    /// Raw markdown text slice for this chunk (headings included in body for embedding quality).
    /// Frontmatter is excluded. Chunks may overlap (CHUNK_OVERLAP_CHARS).
    pub body: String,
    /// Start line number in the source file (1-based)
    pub line_start: usize,
    /// End line number in the source file (1-based)
    pub line_end: usize,
    /// Frontmatter metadata from the file (shared across all chunks from same file)
    pub frontmatter: Frontmatter,
}

/// Result of chunking a single file
#[derive(Debug)]
pub struct ChunkedFile {
    pub file_path: PathBuf,
    pub frontmatter: Frontmatter,
    pub chunks: Vec<Chunk>,
}

/// Result of embedding a batch of texts via an embedding provider.
#[derive(Debug, Clone)]
pub struct EmbeddingResult {
    pub embeddings: Vec<Vec<f32>>,
    pub model: String,
    pub total_tokens: u64,
}
