//! Web dashboard integration tests (Wave 0 stubs)
//!
//! These stubs exist to satisfy the Nyquist validation contract.
//! Each test will be implemented as the corresponding plan delivers the handler.

#[test]
#[ignore = "Plan 01: awaiting askama + axum wiring"]
fn test_base_template_compiles() {
    // WEB-01: Verify base template renders with nav bar and brand
}

#[test]
#[ignore = "Plan 01: awaiting serve command implementation"]
fn test_serve_command_binds() {
    // CLI-05: Verify serve command starts HTTP listener
}

#[test]
#[ignore = "Plan 01: awaiting dashboard router"]
fn test_router_has_all_routes() {
    // WEB-01: Verify dashboard_router includes /, /search, /index, /status, /settings
}

#[test]
#[ignore = "Plan 02: awaiting search handler"]
fn test_search_handler_empty_query() {
    // WEB-02: Empty query returns form only, no results
}

#[test]
#[ignore = "Plan 02: awaiting search handler"]
fn test_search_handler_with_query() {
    // WEB-02: Query returns results with file path, breadcrumb, score, chunk text
}

#[test]
#[ignore = "Plan 03: awaiting index handler"]
fn test_index_browser_lists_files() {
    // WEB-03: Index page shows per-file chunk count and last-indexed timestamp
}

#[test]
#[ignore = "Plan 03: awaiting status handler"]
fn test_status_handler_shows_stats() {
    // WEB-04: Status shows total chunks/files, queue depth, embedding stats
}

#[test]
#[ignore = "Plan 03: awaiting status handler"]
fn test_status_handler_shows_token_usage() {
    // WEB-05: Status shows estimated token usage (N/A in v1)
}

#[test]
#[ignore = "Plan 03: awaiting settings handler"]
fn test_settings_handler_shows_config() {
    // WEB-05/WEB-06: Settings shows config values and credential source (not key)
}
