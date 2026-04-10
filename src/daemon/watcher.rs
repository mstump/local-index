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
                    // blocking_send because this callback runs on notify's sync thread
                    let _ = tx.blocking_send(events);
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
