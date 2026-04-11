use notify::event::{ModifyKind, RenameMode};
use notify::EventKind;
use notify_debouncer_full::DebouncedEvent;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::daemon::metrics;
use crate::pipeline::chunker::chunk_markdown;
use crate::pipeline::embedder::Embedder;
use crate::pipeline::store::{compute_content_hash, ChunkStore};
use crate::search::SearchEngine;

/// Run the event processing loop. Receives batches of DebouncedEvents from the
/// watcher channel, processes each event (create -> index, modify -> re-index,
/// delete -> remove chunks, rename -> delete old + index new), and rebuilds the
/// FTS index after each batch.
///
/// Runs until the CancellationToken is cancelled or the channel closes.
pub async fn run_event_processor<E: Embedder>(
    mut rx: mpsc::Receiver<Vec<DebouncedEvent>>,
    vault_path: PathBuf,
    store: ChunkStore,
    embedder: E,
    token: CancellationToken,
) {
    tracing::info!("event processor started");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                tracing::info!("event processor shutting down");
                break;
            }
            Some(events) = rx.recv() => {
                let batch_start = Instant::now();
                let mut chunks_processed: u64 = 0;

                for event in &events {
                    // Only process .md files
                    let md_paths: Vec<&Path> = event.paths.iter()
                        .map(|p| p.as_path())
                        .filter(|p| p.extension().map(|e| e == "md").unwrap_or(false))
                        .collect();

                    if md_paths.is_empty() {
                        continue;
                    }

                    // T-04-05: Validate all paths are within vault_path
                    // Skip any path that cannot be resolved relative to the vault
                    let valid_paths: Vec<&Path> = md_paths.iter()
                        .copied()
                        .filter(|p| p.strip_prefix(&vault_path).is_ok())
                        .collect();

                    if valid_paths.is_empty() {
                        tracing::warn!(paths = ?md_paths, "skipping event with paths outside vault");
                        continue;
                    }

                    match &event.kind {
                        // WTCH-02: Handle rename events explicitly BEFORE generic Modify.
                        // notify-debouncer-full emits rename events as EventKind::Modify(ModifyKind::Name(_)).
                        // event.paths layout for renames:
                        //   RenameMode::Both => [old_path, new_path]
                        //   RenameMode::From => [old_path] (only old path known)
                        //   RenameMode::To   => [new_path] (only new path known)
                        EventKind::Modify(ModifyKind::Name(rename_mode)) => {
                            match rename_mode {
                                RenameMode::Both => {
                                    // event.paths[0] = old path, event.paths[1] = new path
                                    if event.paths.len() >= 2 {
                                        let old_path = &event.paths[0];
                                        let new_path = &event.paths[1];

                                        // Remove chunks for old path (if it was .md and in vault)
                                        if old_path.extension().map(|e| e == "md").unwrap_or(false)
                                            && old_path.strip_prefix(&vault_path).is_ok()
                                        {
                                            tracing::info!(
                                                old = %old_path.display(),
                                                new = %new_path.display(),
                                                "rename detected, removing old path chunks"
                                            );
                                            remove_file(old_path, &vault_path, &store).await;
                                        }

                                        // Index new path (if it is .md and in vault)
                                        if new_path.extension().map(|e| e == "md").unwrap_or(false)
                                            && new_path.strip_prefix(&vault_path).is_ok()
                                        {
                                            let count = reindex_file(new_path, &vault_path, &store, &embedder).await;
                                            chunks_processed += count;
                                        }

                                        metrics::increment_file_events();
                                    }
                                }
                                RenameMode::From => {
                                    // Only old path available -- file was renamed away. Delete its chunks.
                                    if let Some(old_path) = event.paths.first() {
                                        if old_path.extension().map(|e| e == "md").unwrap_or(false)
                                            && old_path.strip_prefix(&vault_path).is_ok()
                                        {
                                            tracing::info!(
                                                old = %old_path.display(),
                                                "rename-from detected, removing old path chunks"
                                            );
                                            remove_file(old_path, &vault_path, &store).await;
                                            metrics::increment_file_events();
                                        }
                                    }
                                }
                                RenameMode::To => {
                                    // Only new path available -- file was renamed here. Index it.
                                    if let Some(new_path) = event.paths.first() {
                                        if new_path.extension().map(|e| e == "md").unwrap_or(false)
                                            && new_path.strip_prefix(&vault_path).is_ok()
                                        {
                                            tracing::info!(
                                                new = %new_path.display(),
                                                "rename-to detected, indexing new path"
                                            );
                                            let count = reindex_file(new_path, &vault_path, &store, &embedder).await;
                                            chunks_processed += count;
                                            metrics::increment_file_events();
                                        }
                                    }
                                }
                                _ => {
                                    // RenameMode::Any or other -- treat as generic modify on all paths
                                    for path in &valid_paths {
                                        let count = reindex_file(path, &vault_path, &store, &embedder).await;
                                        chunks_processed += count;
                                        metrics::increment_file_events();
                                    }
                                }
                            }
                        }

                        // WTCH-01, WTCH-03: Create and non-rename Modify events trigger re-indexing
                        EventKind::Create(_) | EventKind::Modify(_) => {
                            for path in &valid_paths {
                                let count = reindex_file(path, &vault_path, &store, &embedder).await;
                                chunks_processed += count;
                                metrics::increment_file_events();
                            }
                        }

                        // WTCH-04: Remove events delete all chunks for the file
                        EventKind::Remove(_) => {
                            for path in &valid_paths {
                                remove_file(path, &vault_path, &store).await;
                                metrics::increment_file_events();
                            }
                        }
                        _ => {
                            tracing::debug!(kind = ?event.kind, paths = ?event.paths, "ignoring event");
                        }
                    }
                }

                // Update gauges after batch (uses count_total_chunks and count_distinct_files from Plan 02)
                if let Ok(total) = store.count_total_chunks().await {
                    metrics::set_chunks_total(total as f64);
                }
                if let Ok(files) = store.count_distinct_files().await {
                    metrics::set_files_total(files as f64);
                }

                // Record throughput
                if chunks_processed > 0 {
                    let elapsed = batch_start.elapsed();
                    let throughput = chunks_processed as f64 / elapsed.as_secs_f64();
                    metrics::record_indexing_throughput(throughput);

                    // Rebuild FTS index after changes
                    let engine = SearchEngine::new(&store, &embedder);
                    if let Err(e) = engine.ensure_fts_index().await {
                        tracing::warn!(error = %e, "failed to rebuild FTS index after daemon re-index");
                    }
                }
            }
        }
    }
}

/// Re-index a single file: read, chunk, embed, store. Returns number of chunks processed.
async fn reindex_file<E: Embedder>(
    path: &Path,
    vault_path: &Path,
    store: &ChunkStore,
    embedder: &E,
) -> u64 {
    let relative = path.strip_prefix(vault_path).unwrap_or(path);
    let relative_str = relative.to_string_lossy().to_string();

    tracing::info!(file = %relative_str, "re-indexing file");

    // Read file (async to avoid blocking the executor thread)
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to read file");
            metrics::increment_embedding_errors();
            return 0;
        }
    };

    // Chunk
    let chunked = match chunk_markdown(&content, relative) {
        Ok(cf) => cf,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to chunk file");
            metrics::increment_embedding_errors();
            return 0;
        }
    };

    if chunked.chunks.is_empty() {
        // Delete any existing chunks for this file (file may have been emptied)
        let _ = store.delete_chunks_for_file(&relative_str).await;
        return 0;
    }

    // Embed
    let texts: Vec<String> = chunked.chunks.iter().map(|c| c.body.clone()).collect();
    let embed_start = Instant::now();
    let result = match embedder.embed(&texts).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to embed file");
            metrics::increment_embedding_errors();
            return 0;
        }
    };
    metrics::record_embedding_latency(embed_start.elapsed());

    // Compute hashes
    let hashes: Vec<String> = chunked.chunks.iter().map(|c| compute_content_hash(c)).collect();

    // Delete old + store new
    if let Err(e) = store.delete_chunks_for_file(&relative_str).await {
        tracing::warn!(file = %relative_str, error = %e, "failed to delete old chunks");
    }

    let chunk_count = chunked.chunks.len() as u64;
    match store
        .store_chunks(&chunked.chunks, &result.embeddings, &hashes, embedder.model_id())
        .await
    {
        Ok(()) => {
            metrics::increment_chunks_indexed(chunk_count);
            tracing::info!(file = %relative_str, chunks = chunk_count, "re-indexed file");
            chunk_count
        }
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to store chunks");
            metrics::increment_embedding_errors();
            0
        }
    }
}

/// Remove all chunks for a deleted file.
async fn remove_file(path: &Path, vault_path: &Path, store: &ChunkStore) {
    let relative = path.strip_prefix(vault_path).unwrap_or(path);
    let relative_str = relative.to_string_lossy().to_string();

    tracing::info!(file = %relative_str, "removing chunks for deleted file");

    if let Err(e) = store.delete_chunks_for_file(&relative_str).await {
        // Log only -- this is a store/database error, not an embedding error.
        // Incrementing embedding_errors_total here would mislead operators into
        // investigating the embedding pipeline when the real issue is the database.
        tracing::warn!(file = %relative_str, error = %e, "failed to remove chunks for deleted file");
    }
    // NOTE: do NOT call metrics::increment_file_events() here.
    // All callers of remove_file are responsible for incrementing the counter,
    // consistent with how reindex_file is structured.
}
