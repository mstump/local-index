use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;

/// Set up signal handling. Returns a CancellationToken that is cancelled on SIGINT or SIGTERM.
/// Whichever signal fires first triggers graceful shutdown. A second SIGINT forces immediate exit.
///
/// Note: tokio::signal::unix is Unix-only. This is intentional — the project targets macOS
/// (primary) and Linux (secondary). Windows is not a supported platform.
pub fn setup_shutdown() -> CancellationToken {
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        // Register SIGTERM stream before the select! so the OS queues the signal
        // even if it arrives before we reach the await point.
        let mut sigterm = signal(SignalKind::terminate())
            .expect("failed to install SIGTERM handler");

        // First signal (either SIGINT or SIGTERM): graceful shutdown
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
        tracing::info!("shutdown signal received, draining in-flight work...");
        shutdown_token.cancel();

        // Second SIGINT: force exit (SIGTERM after graceful start is ignored; process will
        // exit naturally once all tasks drain)
        tokio::signal::ctrl_c().await.ok();
        tracing::warn!("second signal received, forcing exit");
        std::process::exit(1);
    });
    token
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Verify that SIGTERM cancels the CancellationToken within 500ms.
    #[tokio::test]
    async fn test_sigterm_cancels_token() {
        let token = setup_shutdown();
        // Give handler task time to schedule
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Send SIGTERM to self
        unsafe { libc::kill(libc::getpid(), libc::SIGTERM) };

        // Token must be cancelled within 500ms
        tokio::time::timeout(Duration::from_millis(500), token.cancelled())
            .await
            .expect("CancellationToken was not cancelled after SIGTERM within 500ms");
    }

    /// Verify that SIGINT still cancels the CancellationToken within 500ms.
    #[tokio::test]
    async fn test_sigint_still_cancels_token() {
        let token = setup_shutdown();
        // Give handler task time to schedule
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Send SIGINT to self
        unsafe { libc::kill(libc::getpid(), libc::SIGINT) };

        // Token must be cancelled within 500ms
        tokio::time::timeout(Duration::from_millis(500), token.cancelled())
            .await
            .expect("CancellationToken was not cancelled after SIGINT within 500ms");
    }
}
