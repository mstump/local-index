use anyhow::Result;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

// Counters (OBS-03)
pub const METRIC_CHUNKS_INDEXED_TOTAL: &str = "chunks_indexed_total";
pub const METRIC_EMBEDDING_ERRORS_TOTAL: &str = "embedding_errors_total";
pub const METRIC_FILE_EVENTS_PROCESSED_TOTAL: &str = "file_events_processed_total";
pub const METRIC_SEARCH_QUERIES_TOTAL: &str = "search_queries_total";

// Gauges (OBS-04)
pub const METRIC_QUEUE_DEPTH: &str = "queue_depth";
pub const METRIC_CHUNKS_TOTAL: &str = "chunks_total";
pub const METRIC_FILES_TOTAL: &str = "files_total";
pub const METRIC_STALE_FILES_TOTAL: &str = "stale_files_total";

// Histograms (OBS-02)
pub const METRIC_EMBEDDING_LATENCY_SECONDS: &str = "embedding_latency_seconds";
pub const METRIC_INDEXING_THROUGHPUT_CPS: &str = "indexing_throughput_chunks_per_second";
pub const METRIC_SEARCH_LATENCY_SECONDS: &str = "search_latency_seconds";
pub const METRIC_HTTP_REQUEST_DURATION_SECONDS: &str = "http_request_duration_seconds";

/// Set up the Prometheus metrics recorder with custom histogram buckets.
/// Must be called once at daemon startup before any metrics are recorded.
pub fn setup_metrics() -> Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(METRIC_EMBEDDING_LATENCY_SECONDS.to_string()),
            &[0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        )?
        .set_buckets_for_metric(
            Matcher::Full(METRIC_SEARCH_LATENCY_SECONDS.to_string()),
            &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0],
        )?
        .set_buckets_for_metric(
            Matcher::Full(METRIC_HTTP_REQUEST_DURATION_SECONDS.to_string()),
            &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0],
        )?
        .set_buckets_for_metric(
            Matcher::Full(METRIC_INDEXING_THROUGHPUT_CPS.to_string()),
            &[1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0],
        )?
        .install_recorder()?;
    Ok(handle)
}

// Convenience recording functions

pub fn record_embedding_latency(duration: std::time::Duration) {
    metrics::histogram!(METRIC_EMBEDDING_LATENCY_SECONDS).record(duration.as_secs_f64());
}

pub fn increment_chunks_indexed(count: u64) {
    metrics::counter!(METRIC_CHUNKS_INDEXED_TOTAL).increment(count);
}

pub fn increment_embedding_errors() {
    metrics::counter!(METRIC_EMBEDDING_ERRORS_TOTAL).increment(1);
}

pub fn increment_file_events() {
    metrics::counter!(METRIC_FILE_EVENTS_PROCESSED_TOTAL).increment(1);
}

pub fn increment_search_queries() {
    metrics::counter!(METRIC_SEARCH_QUERIES_TOTAL).increment(1);
}

pub fn set_queue_depth(depth: f64) {
    metrics::gauge!(METRIC_QUEUE_DEPTH).set(depth);
}

pub fn set_chunks_total(count: f64) {
    metrics::gauge!(METRIC_CHUNKS_TOTAL).set(count);
}

pub fn set_files_total(count: f64) {
    metrics::gauge!(METRIC_FILES_TOTAL).set(count);
}

pub fn set_stale_files_total(count: f64) {
    metrics::gauge!(METRIC_STALE_FILES_TOTAL).set(count);
}

pub fn record_search_latency(duration: std::time::Duration) {
    metrics::histogram!(METRIC_SEARCH_LATENCY_SECONDS).record(duration.as_secs_f64());
}

pub fn record_http_latency(duration: std::time::Duration) {
    metrics::histogram!(METRIC_HTTP_REQUEST_DURATION_SECONDS).record(duration.as_secs_f64());
}

pub fn record_indexing_throughput(chunks_per_second: f64) {
    metrics::histogram!(METRIC_INDEXING_THROUGHPUT_CPS).record(chunks_per_second);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

    fn get_or_init_handle() -> &'static PrometheusHandle {
        HANDLE.get_or_init(|| {
            setup_metrics().expect("setup_metrics should succeed")
        })
    }

    #[test]
    fn test_setup_metrics_returns_handle() {
        let handle = get_or_init_handle();
        let rendered = handle.render();
        assert!(rendered.is_ascii());
    }

    #[test]
    fn test_counter_recording() {
        let handle = get_or_init_handle();
        increment_chunks_indexed(5);
        let rendered = handle.render();
        assert!(
            rendered.contains("chunks_indexed_total"),
            "Expected 'chunks_indexed_total' in rendered output:\n{}",
            rendered
        );
    }

    #[test]
    fn test_histogram_recording() {
        let handle = get_or_init_handle();
        record_embedding_latency(std::time::Duration::from_millis(500));
        let rendered = handle.render();
        assert!(
            rendered.contains("embedding_latency_seconds"),
            "Expected 'embedding_latency_seconds' in rendered output:\n{}",
            rendered
        );
    }
}
