use axum::body::Body;
use axum::http::Request;
use metrics_exporter_prometheus::PrometheusBuilder;
use tower::ServiceExt;

#[tokio::test]
async fn test_daemon_health_and_metrics_endpoints() {
    // Test the HTTP router directly, without full daemon startup.
    // This avoids needing VOYAGE_API_KEY or a real vault directory.
    let recorder = PrometheusBuilder::new().build_recorder();
    let prom_handle = recorder.handle();
    // Don't install globally (would conflict with other tests)

    let app = local_index::daemon::http::metrics_router(prom_handle);

    // Test /health
    let resp = app
        .clone()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert_eq!(&body[..], b"ok");

    // Test /metrics
    let resp = app
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let text = String::from_utf8(body.to_vec()).expect("valid utf8");
    assert!(text.is_ascii(), "metrics output should be ASCII");
}
