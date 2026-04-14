use notify::EventKind;
use notify::event::{ModifyKind, RenameMode};
use notify_debouncer_full::DebouncedEvent;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::daemon::metrics;
use crate::pipeline::chunker::chunk_markdown;
use crate::pipeline::embedder::Embedder;
use crate::pipeline::store::{ChunkStore, compute_content_hash};
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
    store: Arc<ChunkStore>,
    embedder: Arc<E>,
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
                let mut needs_fts_rebuild = false;

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
                                                event = "Renamed",
                                                path = %old_path.strip_prefix(&vault_path).unwrap_or(old_path).display(),
                                                renamed_to = %new_path.strip_prefix(&vault_path).unwrap_or(new_path).display(),
                                                "file event"
                                            );
                                            needs_fts_rebuild |=
                                                remove_file(old_path, &vault_path, &store).await;
                                        }

                                        // Index new path (if it is .md and in vault)
                                        if new_path.extension().map(|e| e == "md").unwrap_or(false)
                                            && new_path.strip_prefix(&vault_path).is_ok()
                                        {
                                            let (count, modified) =
                                                reindex_file(new_path, &vault_path, &store, &embedder).await;
                                            chunks_processed += count;
                                            needs_fts_rebuild |= modified;
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
                                                event = "Renamed",
                                                path = %old_path.strip_prefix(&vault_path).unwrap_or(old_path).display(),
                                                "file event"
                                            );
                                            needs_fts_rebuild |=
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
                                                event = "Created",
                                                path = %new_path.strip_prefix(&vault_path).unwrap_or(new_path).display(),
                                                "file event"
                                            );
                                            let (count, modified) =
                                                reindex_file(new_path, &vault_path, &store, &embedder).await;
                                            chunks_processed += count;
                                            needs_fts_rebuild |= modified;
                                            metrics::increment_file_events();
                                        }
                                    }
                                }
                                _ => {
                                    // RenameMode::Any or other -- treat as generic modify on all paths
                                    for path in &valid_paths {
                                        let (count, modified) =
                                            reindex_file(path, &vault_path, &store, &embedder).await;
                                        chunks_processed += count;
                                        needs_fts_rebuild |= modified;
                                        metrics::increment_file_events();
                                    }
                                }
                            }
                        }

                        // WTCH-01, WTCH-03: Create and non-rename Modify events trigger re-indexing
                        EventKind::Create(_) | EventKind::Modify(_) => {
                            let event_label = match &event.kind {
                                EventKind::Create(_) => "Created",
                                _ => "Modified",
                            };
                            for path in &valid_paths {
                                tracing::info!(
                                    event = event_label,
                                    path = %path.strip_prefix(&vault_path).unwrap_or(path).display(),
                                    "file event"
                                );
                                let (count, modified) =
                                    reindex_file(path, &vault_path, &store, &embedder).await;
                                chunks_processed += count;
                                needs_fts_rebuild |= modified;
                                metrics::increment_file_events();
                            }
                        }

                        // WTCH-04: Remove events delete all chunks for the file
                        EventKind::Remove(_) => {
                            for path in &valid_paths {
                                tracing::info!(
                                    event = "Deleted",
                                    path = %path.strip_prefix(&vault_path).unwrap_or(path).display(),
                                    "file event"
                                );
                                needs_fts_rebuild |= remove_file(path, &vault_path, &store).await;
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

                // Record throughput and rebuild FTS after any table mutation (including
                // deletes with zero new embeddings, which previously skipped FTS rebuild).
                if chunks_processed > 0 || needs_fts_rebuild {
                    let elapsed = batch_start.elapsed();
                    if chunks_processed > 0 {
                        let throughput = chunks_processed as f64 / elapsed.as_secs_f64();
                        metrics::record_indexing_throughput(throughput);
                    }

                    let engine = SearchEngine::new(&store, &embedder);
                    if let Err(e) = engine.ensure_fts_index().await {
                        tracing::warn!(error = %e, "failed to rebuild FTS index after daemon re-index");
                    }
                }
            }
        }
    }
}

/// Re-index a single file: read, chunk, embed, store.
///
/// Returns `(chunks_indexed, store_modified)` so callers can refresh FTS when rows were
/// removed or rewritten even if no new embeddings were produced.
async fn reindex_file<E: Embedder>(
    path: &Path,
    vault_path: &Path,
    store: &ChunkStore,
    embedder: &E,
) -> (u64, bool) {
    let relative = path.strip_prefix(vault_path).unwrap_or(path);
    let relative_str = relative.to_string_lossy().to_string();

    tracing::info!(file = %relative_str, "re-indexing file");

    // Read file (async to avoid blocking the executor thread)
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to read file");
            metrics::increment_embedding_errors();
            return (0, false);
        }
    };

    // Chunk
    let chunked = match chunk_markdown(&content, relative) {
        Ok(cf) => cf,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to chunk file");
            metrics::increment_embedding_errors();
            return (0, false);
        }
    };

    if chunked.chunks.is_empty() {
        // Delete any existing chunks for this file (file may have been emptied)
        let cleared = store.delete_chunks_for_file(&relative_str).await.is_ok();
        tracing::info!(
            path = %relative_str,
            chunks_added = 0u64,
            chunks_removed = 0u64,
            chunks_skipped = 1u64,
            "indexing outcome"
        );
        return (0, cleared);
    }

    // Embed
    let texts: Vec<String> = chunked.chunks.iter().map(|c| c.body.clone()).collect();
    let embed_start = Instant::now();
    let result = match embedder.embed(&texts).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to embed file");
            metrics::increment_embedding_errors();
            return (0, false);
        }
    };
    metrics::record_embedding_latency(embed_start.elapsed());

    // Compute hashes
    let hashes: Vec<String> = chunked
        .chunks
        .iter()
        .map(|c| compute_content_hash(c))
        .collect();

    // Delete old + store new
    let del_ok = store.delete_chunks_for_file(&relative_str).await.is_ok();
    if !del_ok {
        tracing::warn!(file = %relative_str, "failed to delete old chunks before storing new rows");
    }

    let chunk_count = chunked.chunks.len() as u64;
    match store
        .store_chunks(
            &chunked.chunks,
            &result.embeddings,
            &hashes,
            embedder.model_id(),
        )
        .await
    {
        Ok(()) => {
            metrics::increment_chunks_indexed(chunk_count);
            tracing::info!(
                path = %relative_str,
                chunks_added = chunk_count,
                chunks_removed = 0u64,
                chunks_skipped = 0u64,
                "indexing outcome"
            );
            (chunk_count, true)
        }
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to store chunks");
            metrics::increment_embedding_errors();
            (0, del_ok)
        }
    }
}

/// Remove all chunks for a deleted file. Returns whether the store reported a successful delete.
async fn remove_file(path: &Path, vault_path: &Path, store: &ChunkStore) -> bool {
    let relative = path.strip_prefix(vault_path).unwrap_or(path);
    let relative_str = relative.to_string_lossy().to_string();

    tracing::info!(path = %relative_str, "removing chunks for deleted file");

    match store.delete_chunks_for_file(&relative_str).await {
        Ok(()) => {
            tracing::info!(
                path = %relative_str,
                chunks_added = 0u64,
                chunks_removed = 1u64,
                chunks_skipped = 0u64,
                "indexing outcome"
            );
            true
        }
        Err(e) => {
            // Log only -- this is a store/database error, not an embedding error.
            // Incrementing embedding_errors_total here would mislead operators into
            // investigating the embedding pipeline when the real issue is the database.
            tracing::warn!(file = %relative_str, error = %e, "failed to remove chunks for deleted file");
            false
        }
    }
    // NOTE: do NOT call metrics::increment_file_events() here.
    // All callers of remove_file are responsible for incrementing the counter,
    // consistent with how reindex_file is structured.
}
