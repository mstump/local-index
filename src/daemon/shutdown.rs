use tokio_util::sync::CancellationToken;

/// Set up signal handling. Returns a CancellationToken that is cancelled on SIGINT/SIGTERM.
/// Also handles a second SIGINT as forced exit.
pub fn setup_shutdown() -> CancellationToken {
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        // First signal: graceful shutdown
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received, draining in-flight work...");
        shutdown_token.cancel();

        // Second signal: force exit
        tokio::signal::ctrl_c().await.ok();
        tracing::warn!("second signal received, forcing exit");
        std::process::exit(1);
    });
    token
}
