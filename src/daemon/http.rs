use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use metrics_exporter_prometheus::PrometheusHandle;

use crate::web::context::AppState;
use crate::web::handlers;

/// Create an axum Router with /metrics and /health endpoints.
pub fn metrics_router(handle: PrometheusHandle) -> Router {
    let handle = Arc::new(handle);
    Router::new()
        .route(
            "/metrics",
            get({
                let handle = Arc::clone(&handle);
                move || {
                    let rendered = handle.render();
                    std::future::ready(rendered)
                }
            }),
        )
        .route("/health", get(|| async { "ok" }))
}

/// Create the dashboard router with all UI routes.
pub fn dashboard_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(handlers::search_handler))
        .route("/search", get(handlers::search_handler))
        .route("/index", get(handlers::index_handler))
        .route("/status", get(handlers::status_handler))
        .route("/settings", get(handlers::settings_handler))
        .with_state(state)
}

/// Combine metrics and dashboard routers into a single application router.
pub fn app_router(handle: PrometheusHandle, state: Arc<AppState>) -> Router {
    metrics_router(handle).merge(dashboard_router(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use metrics_exporter_prometheus::PrometheusBuilder;
    use tower::ServiceExt;

    fn test_handle() -> PrometheusHandle {
        // Build a recorder without installing it globally -- just for HTTP handler testing
        let recorder = PrometheusBuilder::new().build_recorder();
        recorder.handle()
    }

    #[tokio::test]
    async fn test_health_returns_ok() {
        let app = metrics_router(test_handle());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body[..], b"ok");
    }

    #[tokio::test]
    async fn test_metrics_returns_200() {
        let app = metrics_router(test_handle());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        // Should return valid prometheus text (may be empty if no metrics recorded)
        let text = String::from_utf8(body.to_vec()).expect("valid utf8");
        assert!(text.is_ascii());
    }
}
