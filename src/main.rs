mod cli;

use anyhow::Result;
use clap::Parser;
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
            tracing::info!(
                path = %path.display(),
                force_reindex = force_reindex,
                "index command invoked"
            );
            tracing::warn!("index command not yet implemented");
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
