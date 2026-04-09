use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub extra: HashMap<String, serde_yml::Value>,
}

/// A single chunk extracted from a markdown file
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// Vault-relative file path
    pub file_path: PathBuf,
    /// Heading hierarchy breadcrumb (e.g., "## Goals > ### Q1")
    pub heading_breadcrumb: String,
    /// The heading level of this chunk's immediate heading (0 = pre-heading content)
    pub heading_level: u8,
    /// The chunk's text content (frontmatter stripped, headings stripped)
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
