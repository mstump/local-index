use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use local_index::credentials::OcrProvider;

/// local-index: Semantic search over your markdown vault
///
/// Watches a directory tree, chunks markdown files by heading, embeds via the
/// Anthropic API, and stores everything in embedded LanceDB. Provides full-text
/// and semantic search via CLI, web dashboard, and Claude Code skills.
#[derive(Parser, Debug)]
#[command(name = "local-index", version, about, long_about)]
pub struct Cli {
    /// Set the log level (overrides RUST_LOG)
    #[arg(
        long,
        global = true,
        env = "LOCAL_INDEX_LOG_LEVEL",
        default_value = "info"
    )]
    pub log_level: String,

    /// Directory for index data (LanceDB, tantivy, metadata)
    #[arg(long, global = true, env = "LOCAL_INDEX_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Perform a one-shot full index of a directory tree
    ///
    /// Walks the target directory recursively, discovers all .md files, chunks
    /// them by heading, embeds each chunk via the Anthropic API, and stores
    /// the results in LanceDB. Exits on completion.
    ///
    /// Asset processing (PDFs and images) uses `VOYAGE_API_KEY` for embeddings and
    /// `ANTHROPIC_API_KEY` when a PDF needs vision or when indexing raster images.
    Index {
        /// Path to the directory tree to index (e.g., your Obsidian vault)
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Force re-indexing even if content hashes match
        #[arg(long)]
        force_reindex: bool,

        /// Skip PDF/image preprocessing (markdown-only indexing)
        #[arg(long = "skip-asset-processing", env = "LOCAL_INDEX_SKIP_ASSET_PROCESSING", global = false)]
        skip_asset_processing: bool,

        /// Extra comma-separated globs to exclude (in addition to .gitignore)
        #[arg(
            long = "exclude-asset-glob",
            env = "LOCAL_INDEX_EXCLUDE_ASSET_GLOBS",
            value_delimiter = ',',
            global = false
        )]
        exclude_asset_globs: Vec<String>,

        /// OCR backend for rasterized scanned PDFs (`anthropic` default). Raster images still use Anthropic vision when a key is present.
        #[arg(long = "ocr-provider", value_enum, env = "LOCAL_INDEX_OCR_PROVIDER", global = false)]
        ocr_provider: Option<OcrProvider>,
    },

    /// Start a persistent daemon that watches for file changes
    ///
    /// Watches the target directory for create, modify, rename, and delete
    /// events. Re-indexes affected chunks automatically. Also starts the
    /// HTTP server for the web dashboard and metrics endpoint.
    ///
    /// Asset processing uses `VOYAGE_API_KEY` for embeddings and `ANTHROPIC_API_KEY`
    /// when vision is required for scanned PDFs or images.
    Daemon {
        /// Path to the directory tree to watch (e.g., your Obsidian vault)
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Address to bind the HTTP server to
        #[arg(long, env = "LOCAL_INDEX_BIND", default_value = "127.0.0.1:3000")]
        bind: String,

        /// Skip PDF/image preprocessing (markdown-only indexing)
        #[arg(long = "skip-asset-processing", env = "LOCAL_INDEX_SKIP_ASSET_PROCESSING", global = false)]
        skip_asset_processing: bool,

        /// Extra comma-separated globs to exclude (in addition to .gitignore)
        #[arg(
            long = "exclude-asset-glob",
            env = "LOCAL_INDEX_EXCLUDE_ASSET_GLOBS",
            value_delimiter = ',',
            global = false
        )]
        exclude_asset_globs: Vec<String>,

        /// OCR backend for rasterized scanned PDFs (`anthropic` default). Raster images still use Anthropic vision when a key is present.
        #[arg(long = "ocr-provider", value_enum, env = "LOCAL_INDEX_OCR_PROVIDER", global = false)]
        ocr_provider: Option<OcrProvider>,
    },

    /// Search the indexed vault
    ///
    /// Performs a search query against the index and returns structured JSON
    /// results with chunk text, file path, heading breadcrumb, similarity
    /// score, and frontmatter metadata.
    Search {
        /// The search query string
        #[arg(value_name = "QUERY")]
        query: String,

        /// Maximum number of results to return
        #[arg(long, short = 'n', default_value = "10")]
        limit: usize,

        /// Minimum similarity score threshold (0.0 - 1.0)
        #[arg(long)]
        min_score: Option<f64>,

        /// Search mode
        #[arg(long, default_value = "hybrid")]
        mode: SearchMode,

        /// Filter results to files under this path prefix
        #[arg(long)]
        path_filter: Option<String>,

        /// Filter results to files with this frontmatter tag
        #[arg(long)]
        tag_filter: Option<String>,

        /// Number of surrounding context chunks to include
        #[arg(long, default_value = "0")]
        context: usize,

        /// Output format
        #[arg(long, default_value = "json")]
        format: OutputFormat,

        /// Disable result reranking when a reranker is configured (uses retrieval scores only)
        #[arg(long = "no-rerank", default_value_t = false)]
        no_rerank: bool,
    },

    /// Show index status and statistics
    ///
    /// Displays total indexed chunks, files, last index time, pending queue
    /// depth, stale file count, and embedding model information.
    Status,

    /// Start the HTTP server (dashboard + metrics) without file watching
    ///
    /// Serves the web dashboard and Prometheus metrics endpoint. Does not
    /// watch for file changes -- use `daemon` for combined watching + serving.
    Serve {
        /// Address to bind the HTTP server to
        #[arg(long, env = "LOCAL_INDEX_BIND", default_value = "127.0.0.1:3000")]
        bind: String,
    },
}

/// Search mode selection
#[derive(Debug, Clone, ValueEnum)]
pub enum SearchMode {
    /// Vector similarity search using embeddings
    Semantic,
    /// Full-text search over chunk content
    Fts,
    /// Hybrid search fusing semantic and full-text via RRF (default)
    Hybrid,
}

/// Output format selection
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    /// Machine-readable JSON output (default)
    Json,
    /// Human-readable formatted output
    Pretty,
}
