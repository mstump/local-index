mod cli;

use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use local_index::credentials::{resolve_ocr_provider, resolve_voyage_key};
use local_index::pipeline::assets::{
    build_ocr_and_image_clients, discover_asset_paths, ingest_asset_path,
};
use local_index::pipeline::chunker::chunk_markdown;
use local_index::pipeline::embedder::{Embedder, VoyageEmbedder};
use local_index::pipeline::store::{ChunkStore, compute_content_hash};
use local_index::pipeline::walker::discover_markdown_files;
use std::collections::HashSet;
use std::io::IsTerminal;
use tracing_subscriber::{EnvFilter, fmt};

fn init_logging(log_level: &str) {
    // RUST_LOG takes precedence if set; otherwise use --log-level with LanceDB noise suppressed
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{},lancedb=warn,lance=warn", log_level)));

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
        cli::Command::Index {
            path,
            force_reindex,
            skip_asset_processing,
            exclude_asset_globs,
            ocr_provider,
        } => {
            let vault_path = path
                .canonicalize()
                .map_err(|e| anyhow::anyhow!("Invalid vault path '{}': {}", path.display(), e))?;

            tracing::info!(
                path = %vault_path.display(),
                force_reindex = force_reindex,
                skip_asset_processing = skip_asset_processing,
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
            let total_files = files.len();
            if total_files == 0 {
                tracing::warn!("no markdown files found in vault");
            }

            // Step 7: Create progress reporting
            let is_tty = std::io::stdout().is_terminal();
            let pb = if is_tty && total_files > 0 {
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
            let mut cleared_empty_files = false;

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

                let relative_path = file_path.strip_prefix(&vault_path).unwrap_or(file_path);
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
                    if store
                        .delete_chunks_for_file(&relative_path_str)
                        .await
                        .is_ok()
                    {
                        cleared_empty_files = true;
                    } else {
                        tracing::warn!(
                            file = %relative_path_str,
                            "failed to delete chunks for file that produced no chunks"
                        );
                    }
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
                let computed_hashes: Vec<String> = all_file_chunks
                    .iter()
                    .map(|c| compute_content_hash(c))
                    .collect();
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

            let max_asset_b = std::env::var("LOCAL_INDEX_MAX_ASSET_BYTES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50 * 1024 * 1024);
            let max_pdf_pages = std::env::var("LOCAL_INDEX_MAX_PDF_PAGES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30);

            let mut assets_indexed: usize = 0;
            if *skip_asset_processing {
                tracing::info!("asset processing skipped");
            } else {
                let provider = resolve_ocr_provider(*ocr_provider);
                let (pdf_ocr_owned, anthropic_opt) =
                    build_ocr_and_image_clients(provider).map_err(|e| anyhow::anyhow!("{}", e))?;
                let exts = ["pdf", "png", "jpg", "jpeg", "webp"];
                let globs: Vec<String> = exclude_asset_globs.clone();
                let asset_paths = discover_asset_paths(&vault_path, &exts, &globs)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                tracing::info!(count = asset_paths.len(), "discovered asset files");
                for rel in asset_paths {
                    let rel_str = rel.to_string_lossy().to_string();
                    let cf = ingest_asset_path(
                        &vault_path,
                        &rel,
                        &data_dir,
                        max_asset_b,
                        max_pdf_pages,
                        pdf_ocr_owned.as_ref(),
                        anthropic_opt.as_ref(),
                    )
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                    if cf.chunks.is_empty() {
                        continue;
                    }

                    let texts: Vec<String> = cf.chunks.iter().map(|c| c.body.clone()).collect();
                    match embedder.embed(&texts).await {
                        Ok(result) => {
                            if let Err(e) = store.delete_chunks_for_file(&rel_str).await {
                                tracing::warn!(file = %rel_str, error = %e, "failed to delete old asset chunks");
                            }
                            let hashes: Vec<String> =
                                cf.chunks.iter().map(|c| compute_content_hash(c)).collect();
                            match store
                                .store_chunks(
                                    &cf.chunks,
                                    &result.embeddings,
                                    &hashes,
                                    embedder.model_id(),
                                )
                                .await
                            {
                                Ok(()) => {
                                    chunks_embedded += cf.chunks.len();
                                    assets_indexed += 1;
                                    tracing::info!(
                                        file = %rel_str,
                                        chunks = cf.chunks.len(),
                                        "embedded and stored asset"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(file = %rel_str, error = %e, "failed to store asset chunks");
                                    error_count += 1;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(file = %rel_str, error = %e, "failed to embed asset");
                            error_count += 1;
                        }
                    }
                }
            }

            let pruned_orphans = store
                .prune_absent_markdown_files(&vault_path, &files)
                .await?;

            // Step 9: Finish and output summary
            if let Some(pb) = &pb {
                pb.finish_and_clear();
            }

            if is_tty {
                println!(
                    "Indexed {} files | {} assets | {} chunks embedded | {} skipped | {} errors | {} orphan files removed",
                    file_count, assets_indexed, chunks_embedded, chunks_skipped, error_count, pruned_orphans
                );
            } else {
                let summary = serde_json::json!({
                    "files_indexed": file_count,
                    "assets_indexed": assets_indexed,
                    "chunks_embedded": chunks_embedded,
                    "chunks_skipped": chunks_skipped,
                    "errors": error_count,
                    "orphan_files_removed": pruned_orphans
                });
                println!("{}", serde_json::to_string(&summary)?);
            }

            tracing::info!(
                files = file_count,
                assets_indexed = assets_indexed,
                chunks_embedded = chunks_embedded,
                chunks_skipped = chunks_skipped,
                errors = error_count,
                orphan_files_removed = pruned_orphans,
                "indexing complete"
            );

            // Create/refresh FTS index for search command
            if chunks_embedded > 0 || pruned_orphans > 0 || cleared_empty_files || assets_indexed > 0 {
                tracing::info!("creating FTS index for search");
                let engine = local_index::search::SearchEngine::new(&store, &embedder);
                if let Err(e) = engine.ensure_fts_index().await {
                    tracing::warn!(error = %e, "failed to create FTS index (search may need to create it on first use)");
                } else {
                    tracing::info!("FTS index created successfully");
                }
            }
        }
        cli::Command::Daemon {
            path,
            bind,
            skip_asset_processing,
            exclude_asset_globs,
            ocr_provider,
        } => {
            let vault_path = path
                .canonicalize()
                .map_err(|e| anyhow::anyhow!("Invalid vault path '{}': {}", path.display(), e))?;

            let data_dir = cli
                .data_dir
                .clone()
                .unwrap_or_else(|| vault_path.join(".local-index"));
            let db_path = data_dir.to_string_lossy().to_string();

            tracing::info!(
                path = %vault_path.display(),
                bind = %bind,
                data_dir = %db_path,
                "starting daemon"
            );

            local_index::daemon::run_daemon(
                vault_path,
                bind.clone(),
                db_path,
                *skip_asset_processing,
                exclude_asset_globs.clone(),
                resolve_ocr_provider(*ocr_provider),
            )
            .await?;
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
            no_rerank,
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
                no_rerank = no_rerank,
                "search command invoked"
            );

            // Resolve data directory
            let data_dir = cli
                .data_dir
                .clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap().join(".local-index"));

            if !data_dir.exists() {
                anyhow::bail!(
                    "No index found at '{}'. Run `local-index index <path>` first, or specify --data-dir.",
                    data_dir.display()
                );
            }

            let db_path = data_dir.to_string_lossy().to_string();

            // Open store
            let store = local_index::pipeline::store::ChunkStore::open(&db_path).await?;

            // Resolve credentials and create embedder
            let api_key = resolve_voyage_key()?;
            let embedder = VoyageEmbedder::new(api_key);

            let reranker = local_index::claude_rerank::AnthropicReranker::try_from_env();
            let engine = local_index::search::SearchEngine::new(&store, &embedder)
                .with_anthropic_reranker(reranker);

            // Convert CLI SearchMode to library SearchMode
            let lib_mode = match mode {
                cli::SearchMode::Semantic => local_index::search::SearchMode::Semantic,
                cli::SearchMode::Fts => local_index::search::SearchMode::Fts,
                cli::SearchMode::Hybrid => local_index::search::SearchMode::Hybrid,
            };

            let opts = local_index::search::SearchOptions {
                query: query.clone(),
                limit: *limit,
                min_score: *min_score,
                mode: lib_mode,
                path_filter: path_filter.clone(),
                tag_filter: tag_filter.clone(),
                context: *context,
                rerank: !*no_rerank,
            };

            let response = engine.search(&opts).await?;

            // Format and output
            let output = match format {
                cli::OutputFormat::Json => local_index::search::format_json(&response)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize results: {}", e))?,
                cli::OutputFormat::Pretty => local_index::search::format_pretty(&response),
            };

            println!("{}", output);
        }
        cli::Command::Status => {
            tracing::info!("status command invoked");

            // Resolve data directory (same pattern as search command)
            let data_dir = cli
                .data_dir
                .clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap().join(".local-index"));

            let is_tty = std::io::stdout().is_terminal();

            if !data_dir.exists() {
                if is_tty {
                    println!("No index found at '{}'.", data_dir.display());
                    println!("Run `local-index index <path>` first, or specify --data-dir.");
                } else {
                    let status = serde_json::json!({
                        "error": format!("No index found at '{}'", data_dir.display())
                    });
                    println!("{}", serde_json::to_string(&status)?);
                }
                return Ok(());
            }

            let db_path = data_dir.to_string_lossy().to_string();
            let store = local_index::pipeline::store::ChunkStore::open(&db_path).await?;

            let total_chunks = store.count_total_chunks().await.unwrap_or(0);
            let total_files = store.count_distinct_files().await.unwrap_or(0);

            if is_tty {
                println!("Index Status");
                println!("============");
                println!("Total chunks:     {}", total_chunks);
                println!("Total files:      {}", total_files);
                println!("Last index time:  unknown");
                println!("Queue depth:      0 (daemon not running)");
                println!("Stale files:      0");
                println!("Data directory:   {}", data_dir.display());
            } else {
                let status = serde_json::json!({
                    "total_chunks": total_chunks,
                    "total_files": total_files,
                    "last_index_time": null,
                    "queue_depth": 0,
                    "queue_depth_note": "daemon not running",
                    "stale_files": 0,
                    "data_dir": data_dir.to_string_lossy()
                });
                println!("{}", serde_json::to_string(&status)?);
            }
        }
        cli::Command::Serve { bind } => {
            let data_dir = cli.data_dir.clone().unwrap_or_else(|| {
                // Per CONTEXT.md: defaults to $LOCAL_INDEX_DATA_DIR or ~/.local-index
                if let Ok(env_dir) = std::env::var("LOCAL_INDEX_DATA_DIR") {
                    std::path::PathBuf::from(env_dir)
                } else {
                    dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join(".local-index")
                }
            });
            let db_path = data_dir.to_string_lossy().to_string();

            tracing::info!(bind = %bind, data_dir = %db_path, "starting serve");

            local_index::daemon::run_serve(bind.clone(), db_path, cli.log_level.clone()).await?;
        }
    }

    Ok(())
}
