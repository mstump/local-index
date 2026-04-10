mod cli;

use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use local_index::credentials::resolve_voyage_key;
use local_index::pipeline::chunker::chunk_markdown;
use local_index::pipeline::embedder::{Embedder, VoyageEmbedder};
use local_index::pipeline::store::{compute_content_hash, ChunkStore};
use local_index::pipeline::walker::discover_markdown_files;
use std::collections::HashSet;
use std::io::IsTerminal;
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
        .with_writer(std::io::stderr)
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
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

            // Step 2: Resolve data directory
            let data_dir = cli
                .data_dir
                .clone()
                .unwrap_or_else(|| vault_path.join(".local-index"));
            let db_path = data_dir.to_string_lossy().to_string();

            // Step 3: Resolve credentials
            let api_key = resolve_voyage_key()?;

            // Step 4: Create embedder and store
            let embedder = VoyageEmbedder::new(api_key);
            let store = ChunkStore::open(&db_path).await?;

            // Step 5: Check model consistency
            let needs_full_reindex = store
                .check_model_consistency(embedder.model_id(), *force_reindex)
                .await?;
            if needs_full_reindex {
                tracing::warn!("model changed, clearing all existing data for full re-index");
                store.clear_all().await?;
            }

            // Step 6: Walk and discover files
            let files = discover_markdown_files(&vault_path);

            if files.is_empty() {
                tracing::warn!("no markdown files found in vault");
                let is_tty = std::io::stdout().is_terminal();
                if is_tty {
                    println!("Indexed 0 files | 0 chunks embedded | 0 skipped | 0 errors");
                } else {
                    let summary = serde_json::json!({
                        "files_indexed": 0,
                        "chunks_embedded": 0,
                        "chunks_skipped": 0,
                        "errors": 0
                    });
                    println!("{}", serde_json::to_string(&summary)?);
                }
                return Ok(());
            }

            let total_files = files.len();

            // Step 7: Create progress reporting
            let is_tty = std::io::stdout().is_terminal();
            let pb = if is_tty {
                let pb = ProgressBar::new(total_files as u64);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files ({msg})")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                Some(pb)
            } else {
                None
            };

            // Step 8: Process each file with incremental skip logic
            let mut file_count: usize = 0;
            let mut chunks_embedded: usize = 0;
            let mut chunks_skipped: usize = 0;
            let mut error_count: usize = 0;

            for (file_index, file_path) in files.iter().enumerate() {
                let content = match std::fs::read_to_string(file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(
                            file = %file_path.display(),
                            error = %e,
                            "failed to read file"
                        );
                        error_count += 1;
                        continue;
                    }
                };

                let relative_path = file_path
                    .strip_prefix(&vault_path)
                    .unwrap_or(file_path);
                let relative_path_str = relative_path.to_string_lossy().to_string();

                let chunked_file = match chunk_markdown(&content, relative_path) {
                    Ok(cf) => cf,
                    Err(e) => {
                        tracing::warn!(
                            file = %relative_path_str,
                            error = %e,
                            "failed to chunk file"
                        );
                        error_count += 1;
                        continue;
                    }
                };

                let all_file_chunks = chunked_file.chunks;

                if all_file_chunks.is_empty() {
                    file_count += 1;
                    if let Some(ref pb) = pb {
                        pb.set_message(format!("{} (empty)", relative_path_str));
                        pb.inc(1);
                    } else {
                        eprintln!(
                            "[{}/{}] {} -- 0 chunks embedded, 0 skipped",
                            file_index + 1,
                            total_files,
                            relative_path_str
                        );
                    }
                    continue;
                }

                // Compute content hashes for all chunks
                let computed_hashes: Vec<String> =
                    all_file_chunks.iter().map(|c| compute_content_hash(c)).collect();
                let computed_hash_set: HashSet<&str> =
                    computed_hashes.iter().map(|h| h.as_str()).collect();

                // Get existing hashes from store
                let existing_hashes = store
                    .get_hashes_for_file(&relative_path_str)
                    .await
                    .unwrap_or_default();
                let existing_hash_set: HashSet<&str> =
                    existing_hashes.iter().map(|h| h.as_str()).collect();

                // If hash sets are identical (same hashes, same count), skip the file
                if computed_hash_set == existing_hash_set
                    && computed_hashes.len() == existing_hashes.len()
                {
                    let skipped = all_file_chunks.len();
                    chunks_skipped += skipped;
                    file_count += 1;

                    tracing::debug!(
                        file = %relative_path_str,
                        chunks = skipped,
                        "file unchanged, skipping"
                    );

                    if let Some(ref pb) = pb {
                        pb.set_message(format!("{} (skipped)", relative_path_str));
                        pb.inc(1);
                    } else {
                        eprintln!(
                            "[{}/{}] {} -- 0 chunks embedded, {} skipped",
                            file_index + 1,
                            total_files,
                            relative_path_str,
                            skipped
                        );
                    }
                    continue;
                }

                // File has changes -- embed all chunks for this file
                let texts: Vec<String> = all_file_chunks.iter().map(|c| c.body.clone()).collect();

                match embedder.embed(&texts).await {
                    Ok(result) => {
                        // Delete old chunks for this file, then store new ones
                        if let Err(e) = store.delete_chunks_for_file(&relative_path_str).await {
                            tracing::warn!(
                                file = %relative_path_str,
                                error = %e,
                                "failed to delete old chunks"
                            );
                        }

                        match store
                            .store_chunks(
                                &all_file_chunks,
                                &result.embeddings,
                                &computed_hashes,
                                embedder.model_id(),
                            )
                            .await
                        {
                            Ok(()) => {
                                let embedded = all_file_chunks.len();
                                chunks_embedded += embedded;
                                file_count += 1;

                                tracing::info!(
                                    file = %relative_path_str,
                                    chunks = embedded,
                                    tokens = result.total_tokens,
                                    "embedded and stored file"
                                );

                                if let Some(ref pb) = pb {
                                    pb.set_message(format!(
                                        "{} ({} chunks)",
                                        relative_path_str, embedded
                                    ));
                                    pb.inc(1);
                                } else {
                                    eprintln!(
                                        "[{}/{}] {} -- {} chunks embedded, 0 skipped",
                                        file_index + 1,
                                        total_files,
                                        relative_path_str,
                                        embedded
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    file = %relative_path_str,
                                    error = %e,
                                    "failed to store chunks"
                                );
                                error_count += 1;
                                if let Some(ref pb) = pb {
                                    pb.inc(1);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            file = %relative_path_str,
                            error = %e,
                            "failed to embed file"
                        );
                        error_count += 1;
                        if let Some(ref pb) = pb {
                            pb.inc(1);
                        } else {
                            eprintln!(
                                "[{}/{}] {} -- error: {}",
                                file_index + 1,
                                total_files,
                                relative_path_str,
                                e
                            );
                        }
                    }
                }
            }

            // Step 9: Finish and output summary
            if let Some(pb) = &pb {
                pb.finish_and_clear();
            }

            if is_tty {
                println!(
                    "Indexed {} files | {} chunks embedded | {} skipped | {} errors",
                    file_count, chunks_embedded, chunks_skipped, error_count
                );
            } else {
                let summary = serde_json::json!({
                    "files_indexed": file_count,
                    "chunks_embedded": chunks_embedded,
                    "chunks_skipped": chunks_skipped,
                    "errors": error_count
                });
                println!("{}", serde_json::to_string(&summary)?);
            }

            tracing::info!(
                files = file_count,
                chunks_embedded = chunks_embedded,
                chunks_skipped = chunks_skipped,
                errors = error_count,
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
