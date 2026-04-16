pub mod http;
pub mod metrics;
pub mod processor;
pub mod shutdown;
pub mod watcher;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::task::TaskTracker;

use crate::claude_rerank::AnthropicReranker;
use crate::credentials::resolve_voyage_key;
use crate::pipeline::assets::AnthropicAssetClient;
use crate::pipeline::embedder::VoyageEmbedder;
use crate::pipeline::store::ChunkStore;
use crate::web::context::{AppState, DashboardConfig};

/// Run the daemon: watcher + event processor + HTTP server.
/// Blocks until shutdown signal is received.
pub async fn run_daemon(
    vault_path: PathBuf,
    bind_addr: String,
    data_dir: String,
    skip_asset_processing: bool,
    exclude_asset_globs: Vec<String>,
) -> anyhow::Result<()> {
    // 1. Install metrics recorder FIRST (before any metrics macros are used)
    let prom_handle = metrics::setup_metrics()?;

    // 2. Open store and create embedder (wrapped in Arc for shared ownership)
    let store = Arc::new(ChunkStore::open(&data_dir).await?);
    let api_key = resolve_voyage_key()?;
    let embedder = Arc::new(VoyageEmbedder::new(api_key));
    let data_dir_path = PathBuf::from(&data_dir);
    let anthropic = if skip_asset_processing {
        None
    } else {
        AnthropicAssetClient::new_from_env().ok().map(Arc::new)
    };

    // 3. Set initial gauge values (count_total_chunks and count_distinct_files from Plan 02)
    if let Ok(total) = store.count_total_chunks().await {
        metrics::set_chunks_total(total as f64);
    }
    if let Ok(files) = store.count_distinct_files().await {
        metrics::set_files_total(files as f64);
    }
    metrics::set_queue_depth(0.0);

    // 4. Build AppState for dashboard
    let config = Arc::new(DashboardConfig {
        data_dir: data_dir.clone(),
        bind_addr: bind_addr.clone(),
        log_level: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        credential_source: "VOYAGE_API_KEY env var".to_string(),
        embedding_provider: "voyage".to_string(),
        embedding_model: "voyage-3.5".to_string(),
        embedding_dimensions: 1024,
    });
    let anthropic_reranker = AnthropicReranker::try_from_env();
    let app_state = Arc::new(AppState {
        store: Arc::clone(&store),
        embedder: Arc::clone(&embedder),
        config,
        anthropic_reranker,
    });

    // 5. Set up shutdown coordination
    let token = shutdown::setup_shutdown();
    let tracker = TaskTracker::new();

    // 6. Set up file watcher with channel bridge
    let (event_tx, event_rx) = mpsc::channel(256);
    let _watcher = watcher::FileWatcher::new(&vault_path, event_tx)?;

    tracing::info!(path = %vault_path.display(), bind = %bind_addr, "daemon started");

    // 7. Spawn event processor
    let proc_token = token.clone();
    let proc_store = Arc::clone(&store);
    let proc_embedder = Arc::clone(&embedder);
    let proc_data_dir = data_dir_path.clone();
    let proc_anthropic = anthropic.clone();
    let proc_exclude = exclude_asset_globs.clone();
    tracker.spawn(async move {
        processor::run_event_processor(
            event_rx,
            vault_path,
            proc_data_dir,
            proc_store,
            proc_embedder,
            proc_anthropic,
            skip_asset_processing,
            proc_exclude,
            proc_token,
        )
        .await;
    });

    // 8. Spawn HTTP server with combined metrics + dashboard router
    let http_token = token.clone();
    let bind_clone = bind_addr.clone();
    tracker.spawn(async move {
        let app = http::app_router(prom_handle, app_state);
        let listener = match tokio::net::TcpListener::bind(&bind_clone).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(error = %e, bind = %bind_clone, "failed to bind HTTP server");
                // Cancel the token so all other tasks shut down cleanly rather than
                // silently running with no observability endpoints.
                http_token.cancel();
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

    // 9. Wait for all tasks to complete after shutdown
    token.cancelled().await;
    tracker.close();
    tracker.wait().await;

    tracing::info!("daemon shutdown complete");
    Ok(())
}

/// Run the HTTP server (dashboard + metrics) without file watching.
/// Used by the `serve` command for read-only dashboard access.
pub async fn run_serve(
    bind_addr: String,
    data_dir: String,
    log_level: String,
) -> anyhow::Result<()> {
    // 1. Install metrics recorder
    let prom_handle = metrics::setup_metrics()?;

    // 2. Open store (creates empty DB if not found, matching index command behavior)
    let store = Arc::new(ChunkStore::open(&data_dir).await?);

    // 3. Resolve credentials
    let api_key = resolve_voyage_key()?;
    let embedder = Arc::new(VoyageEmbedder::new(api_key));

    // 4. Set initial gauge values
    if let Ok(total) = store.count_total_chunks().await {
        metrics::set_chunks_total(total as f64);
    }
    if let Ok(files) = store.count_distinct_files().await {
        metrics::set_files_total(files as f64);
    }

    // 5. Build AppState
    let config = Arc::new(DashboardConfig {
        data_dir: data_dir.clone(),
        bind_addr: bind_addr.clone(),
        log_level,
        credential_source: "VOYAGE_API_KEY env var".to_string(),
        embedding_provider: "voyage".to_string(),
        embedding_model: "voyage-3.5".to_string(),
        embedding_dimensions: 1024,
    });
    let anthropic_reranker = AnthropicReranker::try_from_env();
    let app_state = Arc::new(AppState {
        store,
        embedder,
        config,
        anthropic_reranker,
    });

    // 6. Build router and serve
    let app = http::app_router(prom_handle, app_state);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!(bind = %bind_addr, "serve: HTTP server listening");

    // 7. Graceful shutdown on Ctrl+C
    let shutdown_token = shutdown::setup_shutdown();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { shutdown_token.cancelled().await })
        .await?;

    tracing::info!("serve: shutdown complete");
    Ok(())
}
