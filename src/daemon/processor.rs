use ignore::overrides::Override;
use notify::EventKind;
use notify::event::{ModifyKind, RenameMode};
use notify_debouncer_full::DebouncedEvent;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::daemon::metrics;
use crate::pipeline::assets::{
    build_asset_exclude_override, ingest_asset_path, is_asset_path_excluded_by_override,
    AnthropicAssetClient,
};
use crate::pipeline::chunker::chunk_markdown;
use crate::pipeline::embedder::Embedder;
use crate::pipeline::store::{ChunkStore, compute_content_hash};
use crate::search::SearchEngine;

fn is_markdown(p: &Path) -> bool {
    p.extension().map(|e| e == "md").unwrap_or(false)
}

fn is_asset(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "pdf" | "png" | "jpg" | "jpeg" | "webp"
            )
        })
        .unwrap_or(false)
}

fn is_tracked(p: &Path) -> bool {
    is_markdown(p) || is_asset(p)
}

/// Run the event processing loop. Receives batches of DebouncedEvents from the
/// watcher channel, processes each event (create -> index, modify -> re-index,
/// delete -> remove chunks, rename -> delete old + index new), and rebuilds the
/// FTS index after each batch.
///
/// Runs until the CancellationToken is cancelled or the channel closes.
pub async fn run_event_processor<E: Embedder>(
    mut rx: mpsc::Receiver<Vec<DebouncedEvent>>,
    vault_path: PathBuf,
    data_dir: PathBuf,
    store: Arc<ChunkStore>,
    embedder: Arc<E>,
    anthropic: Option<Arc<AnthropicAssetClient>>,
    skip_assets: bool,
    exclude_globs: Vec<String>,
    token: CancellationToken,
) {
    tracing::info!("event processor started");

    let vault_path = std::fs::canonicalize(&vault_path).unwrap_or(vault_path);
    let exclude_override = build_asset_exclude_override(&vault_path, &exclude_globs).unwrap_or_else(
        |e| {
            tracing::error!(
                error = %e,
                "invalid exclude_asset_globs; operator asset excludes disabled"
            );
            Override::empty()
        },
    );

    let max_asset_b = std::env::var("LOCAL_INDEX_MAX_ASSET_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50 * 1024 * 1024);
    let max_pdf_pages = std::env::var("LOCAL_INDEX_MAX_PDF_PAGES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

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
                    let tracked_paths: Vec<&Path> = event.paths.iter()
                        .map(|p| p.as_path())
                        .filter(|p| is_tracked(p))
                        .collect();

                    if tracked_paths.is_empty() {
                        continue;
                    }

                    let valid_paths: Vec<&Path> = tracked_paths.iter()
                        .copied()
                        .filter(|p| p.strip_prefix(&vault_path).is_ok())
                        .collect();

                    if valid_paths.is_empty() {
                        tracing::warn!(paths = ?tracked_paths, "skipping event with paths outside vault");
                        continue;
                    }

                    match &event.kind {
                        EventKind::Modify(ModifyKind::Name(rename_mode)) => {
                            match rename_mode {
                                RenameMode::Both => {
                                    if event.paths.len() >= 2 {
                                        let old_path = &event.paths[0];
                                        let new_path = &event.paths[1];

                                        if is_tracked(old_path) && old_path.strip_prefix(&vault_path).is_ok() {
                                            tracing::info!(
                                                event = "Renamed",
                                                path = %old_path.strip_prefix(&vault_path).unwrap_or(old_path).display(),
                                                renamed_to = %new_path.strip_prefix(&vault_path).unwrap_or(new_path).display(),
                                                "file event"
                                            );
                                            needs_fts_rebuild |=
                                                remove_file(old_path, &vault_path, &store).await;
                                        }

                                        if is_tracked(new_path) && new_path.strip_prefix(&vault_path).is_ok() {
                                            let (count, modified) = reindex_tracked_path(
                                                new_path,
                                                &vault_path,
                                                &data_dir,
                                                &store,
                                                &embedder,
                                                anthropic.as_ref(),
                                                skip_assets,
                                                &exclude_override,
                                                max_asset_b,
                                                max_pdf_pages,
                                            )
                                            .await;
                                            chunks_processed += count;
                                            needs_fts_rebuild |= modified;
                                        }

                                        metrics::increment_file_events();
                                    }
                                }
                                RenameMode::From => {
                                    if let Some(old_path) = event.paths.first() {
                                        if is_tracked(old_path) && old_path.strip_prefix(&vault_path).is_ok() {
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
                                    if let Some(new_path) = event.paths.first() {
                                        if is_tracked(new_path) && new_path.strip_prefix(&vault_path).is_ok() {
                                            tracing::info!(
                                                event = "Created",
                                                path = %new_path.strip_prefix(&vault_path).unwrap_or(new_path).display(),
                                                "file event"
                                            );
                                            let (count, modified) = reindex_tracked_path(
                                                new_path,
                                                &vault_path,
                                                &data_dir,
                                                &store,
                                                &embedder,
                                                anthropic.as_ref(),
                                                skip_assets,
                                                &exclude_override,
                                                max_asset_b,
                                                max_pdf_pages,
                                            )
                                            .await;
                                            chunks_processed += count;
                                            needs_fts_rebuild |= modified;
                                            metrics::increment_file_events();
                                        }
                                    }
                                }
                                _ => {
                                    for path in &valid_paths {
                                        let (count, modified) = reindex_tracked_path(
                                            path,
                                            &vault_path,
                                            &data_dir,
                                            &store,
                                            &embedder,
                                            anthropic.as_ref(),
                                            skip_assets,
                                            &exclude_override,
                                            max_asset_b,
                                            max_pdf_pages,
                                        )
                                        .await;
                                        chunks_processed += count;
                                        needs_fts_rebuild |= modified;
                                        metrics::increment_file_events();
                                    }
                                }
                            }
                        }

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
                                let (count, modified) = reindex_tracked_path(
                                    path,
                                    &vault_path,
                                    &data_dir,
                                    &store,
                                    &embedder,
                                    anthropic.as_ref(),
                                    skip_assets,
                                    &exclude_override,
                                    max_asset_b,
                                    max_pdf_pages,
                                )
                                .await;
                                chunks_processed += count;
                                needs_fts_rebuild |= modified;
                                metrics::increment_file_events();
                            }
                        }

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

                if let Ok(total) = store.count_total_chunks().await {
                    metrics::set_chunks_total(total as f64);
                }
                if let Ok(files) = store.count_distinct_files().await {
                    metrics::set_files_total(files as f64);
                }

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

async fn reindex_tracked_path<E: Embedder>(
    path: &Path,
    vault_path: &Path,
    data_dir: &Path,
    store: &ChunkStore,
    embedder: &E,
    anthropic: Option<&Arc<AnthropicAssetClient>>,
    skip_assets: bool,
    exclude_override: &Override,
    max_asset_b: usize,
    max_pdf_pages: usize,
) -> (u64, bool) {
    if is_markdown(path) {
        return reindex_file(path, vault_path, store, embedder).await;
    }
    if is_asset(path) {
        if skip_assets {
            tracing::info!(path = %path.display(), "asset processing skipped");
            return (0, false);
        }
        return reindex_asset(
            path,
            vault_path,
            data_dir,
            store,
            embedder,
            anthropic.map(|a| a.as_ref()),
            exclude_override,
            max_asset_b,
            max_pdf_pages,
        )
        .await;
    }
    (0, false)
}

async fn reindex_asset<E: Embedder>(
    path: &Path,
    vault_path: &Path,
    data_dir: &Path,
    store: &ChunkStore,
    embedder: &E,
    anthropic: Option<&AnthropicAssetClient>,
    exclude_override: &Override,
    max_asset_b: usize,
    max_pdf_pages: usize,
) -> (u64, bool) {
    if is_asset_path_excluded_by_override(vault_path, path, exclude_override) {
        tracing::info!(path = %path.display(), "asset excluded by operator glob");
        return (0, false);
    }
    let relative = path.strip_prefix(vault_path).unwrap_or(path);
    let relative_str = relative.to_string_lossy().to_string();

    let cf = match ingest_asset_path(
        vault_path,
        relative,
        data_dir,
        max_asset_b,
        max_pdf_pages,
        anthropic,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to ingest asset");
            metrics::increment_embedding_errors();
            return (0, false);
        }
    };

    if cf.chunks.is_empty() {
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

    let texts: Vec<String> = cf.chunks.iter().map(|c| c.body.clone()).collect();
    let embed_start = Instant::now();
    let result = match embedder.embed(&texts).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to embed asset");
            metrics::increment_embedding_errors();
            return (0, false);
        }
    };
    metrics::record_embedding_latency(embed_start.elapsed());

    let hashes: Vec<String> = cf.chunks.iter().map(|c| compute_content_hash(c)).collect();
    let del_ok = store.delete_chunks_for_file(&relative_str).await.is_ok();
    if !del_ok {
        tracing::warn!(file = %relative_str, "failed to delete old asset chunks before storing new rows");
    }

    let chunk_count = cf.chunks.len() as u64;
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
            tracing::warn!(file = %relative_str, error = %e, "failed to store asset chunks");
            metrics::increment_embedding_errors();
            (0, del_ok)
        }
    }
}

async fn reindex_file<E: Embedder>(
    path: &Path,
    vault_path: &Path,
    store: &ChunkStore,
    embedder: &E,
) -> (u64, bool) {
    let relative = path.strip_prefix(vault_path).unwrap_or(path);
    let relative_str = relative.to_string_lossy().to_string();

    tracing::info!(file = %relative_str, "re-indexing file");

    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to read file");
            metrics::increment_embedding_errors();
            return (0, false);
        }
    };

    let chunked = match chunk_markdown(&content, relative) {
        Ok(cf) => cf,
        Err(e) => {
            tracing::warn!(file = %relative_str, error = %e, "failed to chunk file");
            metrics::increment_embedding_errors();
            return (0, false);
        }
    };

    if chunked.chunks.is_empty() {
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

    let hashes: Vec<String> = chunked
        .chunks
        .iter()
        .map(|c| compute_content_hash(c))
        .collect();

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
            tracing::warn!(file = %relative_str, error = %e, "failed to remove chunks for deleted file");
            false
        }
    }
}
