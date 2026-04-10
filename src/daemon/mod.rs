pub mod http;
pub mod metrics;
pub mod processor;
pub mod shutdown;
pub mod watcher;

use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::task::TaskTracker;

use crate::credentials::resolve_voyage_key;
use crate::pipeline::embedder::VoyageEmbedder;
use crate::pipeline::store::ChunkStore;

/// Run the daemon: watcher + event processor + HTTP server.
/// Blocks until shutdown signal is received.
pub async fn run_daemon(
    vault_path: PathBuf,
    bind_addr: String,
    data_dir: String,
) -> anyhow::Result<()> {
    // 1. Install metrics recorder FIRST (before any metrics macros are used)
    let prom_handle = metrics::setup_metrics()?;

    // 2. Open store and create embedder
    let store = ChunkStore::open(&data_dir).await?;
    let api_key = resolve_voyage_key()?;
    let embedder = VoyageEmbedder::new(api_key);

    // 3. Set initial gauge values (count_total_chunks and count_distinct_files from Plan 02)
    if let Ok(total) = store.count_total_chunks().await {
        metrics::set_chunks_total(total as f64);
    }
    if let Ok(files) = store.count_distinct_files().await {
        metrics::set_files_total(files as f64);
    }
    metrics::set_queue_depth(0.0);

    // 4. Set up shutdown coordination
    let token = shutdown::setup_shutdown();
    let tracker = TaskTracker::new();

    // 5. Set up file watcher with channel bridge
    let (event_tx, event_rx) = mpsc::channel(256);
    let _watcher = watcher::FileWatcher::new(&vault_path, event_tx)?;

    tracing::info!(path = %vault_path.display(), bind = %bind_addr, "daemon started");

    // 6. Spawn event processor
    let proc_token = token.clone();
    tracker.spawn(async move {
        processor::run_event_processor(event_rx, vault_path, store, embedder, proc_token).await;
    });

    // 7. Spawn HTTP server
    let http_token = token.clone();
    let bind_clone = bind_addr.clone();
    tracker.spawn(async move {
        let app = http::metrics_router(prom_handle);
        let listener = match tokio::net::TcpListener::bind(&bind_clone).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(error = %e, bind = %bind_clone, "failed to bind HTTP server");
                return;
            }
        };
        tracing::info!(bind = %bind_clone, "HTTP server listening");
        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(async move { http_token.cancelled().await })
            .await
        {
            tracing::error!(error = %e, "HTTP server error");
        }
    });

    // 8. Wait for all tasks to complete after shutdown
    token.cancelled().await;
    tracker.close();
    tracker.wait().await;

    tracing::info!("daemon shutdown complete");
    Ok(())
}
