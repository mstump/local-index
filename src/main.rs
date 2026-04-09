mod cli;

use anyhow::Result;
use clap::Parser;
use local_index::pipeline::chunker::chunk_markdown;
use local_index::pipeline::walker::discover_markdown_files;
use tracing_subscriber::{fmt, EnvFilter};

fn init_logging(log_level: &str) {
    // RUST_LOG takes precedence if set; otherwise use --log-level
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .init();
}

fn main() -> Result<()> {
    // Load .env file if present (before clap parses, so env vars are available)
    let _ = dotenvy::dotenv();

    let cli = cli::Cli::parse();

    // Initialize structured logging
    init_logging(&cli.log_level);

    tracing::debug!(log_level = %cli.log_level, "logging initialized");

    match &cli.command {
        cli::Command::Index { path, force_reindex } => {
            let vault_path = path.canonicalize().map_err(|e| {
                anyhow::anyhow!("Invalid vault path '{}': {}", path.display(), e)
            })?;

            tracing::info!(
                path = %vault_path.display(),
                force_reindex = force_reindex,
                "starting index of vault"
            );

            let files = discover_markdown_files(&vault_path);

            if files.is_empty() {
                tracing::warn!("no markdown files found in vault");
                println!("Indexed 0 files, 0 chunks");
                return Ok(());
            }

            let mut total_chunks: usize = 0;
            let mut file_count: usize = 0;
            let mut all_chunks = Vec::new();

            for file_path in &files {
                let content = match std::fs::read_to_string(file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(
                            file = %file_path.display(),
                            error = %e,
                            "failed to process file"
                        );
                        continue;
                    }
                };

                let relative_path = file_path
                    .strip_prefix(&vault_path)
                    .unwrap_or(file_path);

                match chunk_markdown(&content, relative_path) {
                    Ok(chunked_file) => {
                        tracing::info!(
                            file = %relative_path.display(),
                            chunks = chunked_file.chunks.len(),
                            "chunked file"
                        );
                        total_chunks += chunked_file.chunks.len();
                        file_count += 1;
                        all_chunks.extend(chunked_file.chunks);
                    }
                    Err(e) => {
                        tracing::warn!(
                            file = %relative_path.display(),
                            error = %e,
                            "failed to process file"
                        );
                    }
                }
            }

            println!("Indexed {} files, {} chunks", file_count, total_chunks);

            for chunk in &all_chunks {
                println!("{}", serde_json::to_string(chunk)?);
            }

            tracing::info!(
                files = file_count,
                chunks = total_chunks,
                "indexing complete"
            );
        }
        cli::Command::Daemon { path, bind } => {
            tracing::info!(
                path = %path.display(),
                bind = %bind,
                "daemon command invoked"
            );
            tracing::warn!("daemon command not yet implemented");
        }
        cli::Command::Search {
            query,
            limit,
            min_score,
            mode,
            path_filter,
            tag_filter,
            context,
            format,
        } => {
            tracing::info!(
                query = %query,
                limit = limit,
                min_score = ?min_score,
                mode = ?mode,
                path_filter = ?path_filter,
                tag_filter = ?tag_filter,
                context = context,
                format = ?format,
                "search command invoked"
            );
            tracing::warn!("search command not yet implemented");
        }
        cli::Command::Status => {
            tracing::info!("status command invoked");
            tracing::warn!("status command not yet implemented");
        }
        cli::Command::Serve { bind } => {
            tracing::info!(bind = %bind, "serve command invoked");
            tracing::warn!("serve command not yet implemented");
        }
    }

    Ok(())
}
