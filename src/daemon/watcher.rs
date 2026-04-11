use notify::RecursiveMode;
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, Debouncer, RecommendedCache,
};
use notify::RecommendedWatcher;
use notify_debouncer_full::DebouncedEvent;
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct FileWatcher {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

impl FileWatcher {
    /// Start watching `vault_path` recursively. File events are sent to `tx`.
    /// Uses a 500ms debounce window to coalesce rapid events.
    pub fn new(
        vault_path: &Path,
        tx: mpsc::Sender<Vec<DebouncedEvent>>,
    ) -> anyhow::Result<Self> {
        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: DebounceEventResult| {
                if let Ok(events) = result {
                    // try_send avoids blocking notify's internal thread when the
                    // processor is slow. Dropped batches are logged so operators
                    // can detect back-pressure without the watcher thread stalling.
                    if tx.try_send(events).is_err() {
                        tracing::warn!("file event channel full or closed, dropping event batch");
                    }
                }
            },
        )?;
        debouncer.watch(vault_path, RecursiveMode::Recursive)?;
        tracing::info!(path = %vault_path.display(), "file watcher started");
        Ok(Self {
            _debouncer: debouncer,
        })
    }
}
