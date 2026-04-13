# Phase 05: Web Dashboard — Research

**Researched:** 2026-04-10
**Domain:** Rust/axum web dashboard, askama templates, HTML-only UI
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Search interactivity:** Pure HTML form — zero JavaScript. The search UI uses a standard HTML
`<form method="GET" action="/search">` with a text input, a mode selector (hybrid / semantic /
fts), and a submit button. Each search triggers a full-page reload. No JavaScript ships with
the dashboard.

**Navigation structure:** Separate routes — one axum handler per view.

| Route | View |
|-------|------|
| `GET /` | Search UI (default, accepts `?q=&mode=`) |
| `GET /search` | Alias for `/` with query params |
| `GET /index` | File list: per-file chunk count + last-indexed timestamp |
| `GET /status` | Queue depth, last-index time, total chunks/files |
| `GET /settings` | Read-only: config values, credential source, active flags |

**`serve` command wiring:** `--data-dir` flag on the `serve` command. Defaults to
`$LOCAL_INDEX_DATA_DIR` or `~/.local-index`. `serve` opens a `ChunkStore` in read-only mode.
If no LanceDB data exists, initialize empty DB and show "No documents indexed yet" empty state.

**Status view freshness:** Manual reload only. No meta-refresh, no JS polling.

### Claude's Discretion

(No discretion areas specified — all key choices are locked in CONTEXT.md and UI-SPEC.md.)

### Deferred Ideas (OUT OF SCOPE)

- Auto-refresh / live status updates (WebSocket or SSE)
- Dark/light mode toggle
- Authentication or access control
- Mobile-optimized layout
- Search result pagination (v1: just use `--limit`)
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-05 | Operator can run `local-index serve` to start the HTTP server (web dashboard + metrics) without the file watcher | `serve` arm exists in `cli.rs` and `main.rs` but is a stub (just logs a warning). Needs a `run_serve()` function parallel to `run_daemon()`, wiring up the axum router with dashboard routes. |
| WEB-01 | HTTP server serves dashboard on configurable port (default 3000); `--bind` flag | Already in `cli.rs` (`Serve { bind: String }`). Implementation needed in `main.rs` `Serve` arm. |
| WEB-02 | Search UI: text input, mode selector, results list with chunk text, file path, breadcrumb, score | `SearchEngine` and `SearchOptions` are library-ready. Handler needs to own a `ChunkStore` + `VoyageEmbedder`, parse query params, call `engine.search()`, and pass `SearchResponse` to askama template. |
| WEB-03 | Index browser: list of all indexed files with per-file chunk count and last-indexed timestamp | `ChunkStore` has `get_all_file_paths()` and per-file data accessible via LanceDB query. Per-file chunk count: query by `file_path`, count rows. Last-indexed timestamp: NOT in current schema (no timestamp column). Must be addressed — see Gap 1 below. |
| WEB-04 | Status view: total chunks, total files, last full-index time, pending queue depth, stale file count | `count_total_chunks()` and `count_distinct_files()` exist. Queue depth and stale file count need `serve` mode behavior specified (0 for `serve`, live for `daemon`). Last full-index time not persisted — see Gap 1. |
| WEB-05 | Embedding stats: total embeddings, embedding model ID, estimated token usage | Total embeddings = `count_total_chunks()`. Model ID: query any row's `embedding_model` field. Token usage: NOT stored in schema — must show "N/A" or similar. |
| WEB-06 | Read-only settings view: current config values, credential source, active CLI flags | Config is entirely in-memory (parsed from CLI/env at startup). Pass a `SettingsContext` struct to the template. Credential source = whether `VOYAGE_API_KEY` was from env or `.env` file. |
</phase_requirements>

---

## Summary

Phase 5 extends the existing axum HTTP server (already serving `/metrics` and `/health`) with
five dashboard routes. The axum, tokio, and search infrastructure are all in place. The two
significant new pieces are: (1) adding askama as a compile-time HTML templating dependency, and
(2) implementing a `serve` command that mirrors `daemon` but without the file watcher.

The UI design is fully specified in `05-UI-SPEC.md` — pure HTML, system fonts, inline CSS in a
base template, no JavaScript. The planner does not need to make any UI decisions beyond what is
already documented.

There are two schema gaps that affect WEB-03 and WEB-04: the `chunks` table has no timestamp
column, so "last-indexed time" must either be approximated from filesystem metadata, stored in a
separate metadata table, or shown as "unknown" in v1. The plan must decide which. The simplest
v1 approach: show "unknown" for last-indexed time in both the index browser and status view,
documenting this as a known limitation. This avoids schema migration risk.

**Primary recommendation:** Add `askama` (0.15.6) and `askama_web` (0.15.2 with `axum-0.8`
feature) to `Cargo.toml`. Create `templates/` at crate root. Implement five askama templates
extending `base.html`. Wire dashboard routes into a new `dashboard_router()` function. Extend
`metrics_router` call in both `daemon::run_daemon()` and a new `run_serve()` to include dashboard
routes. Implement `serve` command in `main.rs` reusing the existing pattern.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| askama | 0.15.6 | Compile-time HTML templates | Project requirement (CLAUDE.md). Type-safe, zero-runtime overhead, Jinja2-like syntax. Templates checked at compile time. |
| askama_web | 0.15.2 | axum IntoResponse glue | `askama_axum` was deprecated in askama 0.13. `askama_web` is the maintained replacement. Derive `WebTemplate` to get `IntoResponse` for free. |
| axum | 0.8.8 | HTTP routing and handlers | Already in project. No change. |

[VERIFIED: npm registry equivalent — cargo search 2026-04-10]

### Supporting (already in Cargo.toml — no new deps needed)

| Library | Already In | Role in Phase 5 |
|---------|-----------|-----------------|
| tokio | yes | Async runtime for serve command |
| serde / serde_json | yes | Query param deserialization, frontmatter display |
| tracing | yes | Request logging in handlers |
| metrics-exporter-prometheus | yes | PrometheusHandle passed to combined router |

### New Additions

```toml
askama = "0.15"
askama_web = { version = "0.15", features = ["axum-0.8"] }
```

No other new dependencies. The dashboard does not need `tower-http` static file serving because
all CSS is embedded inline in `base.html`.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| askama + askama_web | Manual IntoResponse impl | Manual impl is ~10 lines per template and avoids a dep, but askama_web is idiomatic and maintained |
| askama + askama_web | tera (runtime templates) | CLAUDE.md requires askama compile-time checking |
| askama + askama_web | minijinja | Not mentioned in CLAUDE.md; askama is the locked choice |

---

## Architecture Patterns

### Template Directory Structure

```
templates/
├── base.html       # Layout shell: doctype, head, nav, embedded CSS, main wrapper
├── search.html     # Search form + optional results list (extends base.html)
├── index.html      # File table + empty state (extends base.html)
├── status.html     # Key-value status + embedding stats (extends base.html)
├── settings.html   # Key-value config display (extends base.html)
└── error.html      # 500 error page (extends base.html)
```

Templates live at `templates/` in the crate root (the Cargo.toml package root). Askama resolves
paths relative to the crate root at compile time.
[ASSUMED — standard askama convention; verified via docs.rs and GitHub example]

### Source Module Structure

```
src/
├── daemon/
│   ├── http.rs         # EXTEND: add dashboard_router() alongside metrics_router()
│   └── ...
├── web/                # NEW module
│   ├── mod.rs          # pub use handlers, context types
│   ├── handlers.rs     # axum handler fns for /, /search, /index, /status, /settings
│   ├── context.rs      # Template context structs (SearchContext, IndexContext, etc.)
│   └── error.rs        # AppError implementing IntoResponse
└── main.rs             # EXTEND: Serve command calls run_serve()
```

Alternatively, handlers can live in `src/daemon/http.rs` directly if the module stays small.
Given 5 handlers + 5 context structs, a dedicated `src/web/` module is cleaner.

### Pattern 1: Askama Template with WebTemplate Derive

```rust
// Source: https://github.com/askama-rs/askama_web (verified 2026-04-10)
use askama::Template;
use askama_web::WebTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "search.html")]
pub struct SearchTemplate {
    pub query: Option<String>,
    pub mode: String,
    pub results: Vec<SearchResultView>,
    pub result_count: usize,
    pub active_nav: &'static str,  // "search"
}

async fn search_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<SearchTemplate, AppError> {
    // ... call engine.search(), build SearchTemplate ...
    Ok(SearchTemplate { ... })
}
```

`WebTemplate` derive implements `IntoResponse` automatically. The handler returns
`Result<SearchTemplate, AppError>` — axum handles both arms via `IntoResponse`.
[VERIFIED: cargo search askama_web 0.15.2, askama-rs/askama_web GitHub]

### Pattern 2: Shared AppState via Arc

```rust
// Standard axum pattern [ASSUMED — based on axum 0.8 docs]
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<ChunkStore>,
    pub embedder: Arc<VoyageEmbedder>,
    pub config: Arc<DashboardConfig>,
}

pub struct DashboardConfig {
    pub data_dir: PathBuf,
    pub bind_addr: String,
    pub log_level: String,
    pub credential_source: String,  // e.g. "VOYAGE_API_KEY env var"
}
```

`AppState` is wrapped in `Arc` and passed to the axum router via `.with_state()`. All handlers
receive `State(state): State<Arc<AppState>>` extractor.

### Pattern 3: Query Parameter Deserialization

```rust
// axum 0.8 standard [ASSUMED]
use axum::extract::Query;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub mode: Option<String>,  // "hybrid" | "semantic" | "fts"
}
```

Query params deserialized automatically by serde. `mode` defaults to "hybrid" when absent.

### Pattern 4: Error Handler

```rust
// [ASSUMED — standard axum pattern]
pub enum AppError {
    Search(LocalIndexError),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let template = ErrorTemplate {
            message: self.to_string(),
            active_nav: "",
        };
        (StatusCode::INTERNAL_SERVER_ERROR, Html(template.render().unwrap_or_default()))
            .into_response()
    }
}
```

### Pattern 5: Combining Routers

```rust
// Extend daemon/http.rs [VERIFIED: existing code structure]
pub fn app_router(handle: PrometheusHandle, state: Arc<AppState>) -> Router {
    metrics_router(handle)
        .merge(dashboard_router(state))
}
```

`dashboard_router()` returns a `Router` with `.with_state(state)` applied. The merged router
handles all routes.

### Pattern 6: Template Inheritance

```html
<!-- templates/base.html -->
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>local-index{% block title %} — Dashboard{% endblock %}</title>
  <style>/* inline CSS from UI-SPEC.md */</style>
</head>
<body>
  <nav>...</nav>
  <main>
    {% block content %}{% endblock %}
  </main>
</body>
</html>

<!-- templates/search.html -->
{% extends "base.html" %}
{% block title %} — Search{% endblock %}
{% block content %}
  <form method="GET" action="/search">...</form>
{% endblock %}
```

[ASSUMED — standard askama/Jinja2 inheritance syntax; consistent with askama docs description]

### Pattern 7: Active Nav Link

The `active_nav` field on each template struct is a `&'static str` (e.g., `"search"`, `"index"`).
The base template uses `{% if active_nav == "search" %}class="active"{% endif %}` on each nav
link. This is server-side rendering — no JS required.
[ASSUMED — standard pattern for server-side nav state]

### Anti-Patterns to Avoid

- **Using `askama_axum` crate:** Deprecated since askama 0.13. Use `askama_web` instead.
- **Putting ChunkStore in a Mutex:** `ChunkStore` uses LanceDB's async connection which is
  `Send + Sync`. Wrap in `Arc`, not `Arc<Mutex<>>`.
- **Returning `template.render()` as String directly:** Returns plain text, not HTML. Use
  `WebTemplate` derive or return `Html(template.render()?)`.
- **Embedding credentials in template context:** The `settings` view shows credential SOURCE
  (e.g., "VOYAGE_API_KEY env var") not the credential VALUE. Never pass API key values to
  template context.
- **Using `askama` with `path = "..."` pointing outside `templates/`:** All template paths are
  relative to the `templates/` directory at crate root.

---

## Critical Gap Analysis

### Gap 1: No Timestamp in Schema (affects WEB-03, WEB-04)

**What WEB-03 requires:** Per-file last-indexed timestamp in the index browser.
**What WEB-04 requires:** Last full-index time in the status view.
**Current state:** The `chunks` Arrow schema has NO timestamp column. The `status` CLI command
already shows `"last_index_time": null` — this was an acknowledged gap.

**Options:**

| Approach | Effort | Risk |
|----------|--------|------|
| Show "unknown" / "—" in UI | Zero | None — honest representation of v1 data |
| Add `indexed_at` column to schema, migrate | High — schema migration on LanceDB | Schema migration risk, out of scope for this phase |
| Store last-index-time in a separate sidecar JSON file | Low | Adds a new data file to manage |

**Recommendation:** Show "—" (em-dash) for all timestamps in v1. This is honest and matches the
CLI `status` command behavior. Document as a known v1 limitation. The planner must NOT plan a
schema migration task in this phase.
[VERIFIED: reading src/pipeline/store.rs schema and src/main.rs status output]

### Gap 2: No Per-File Chunk Count Method

**What WEB-03 requires:** Chunk count per file in the index browser.
**Current state:** `ChunkStore` has `count_total_chunks()` and `count_distinct_files()` but NO
per-file chunk count method.

**Solution:** Add `count_chunks_per_file() -> Result<Vec<(String, usize)>, LocalIndexError>` to
`ChunkStore`. Implementation: query `file_path` column for all rows, group by file path in Rust
using a `HashMap<String, usize>`.

This is a low-risk additive change to an existing public method set.
[VERIFIED: reading src/pipeline/store.rs — method does not exist]

### Gap 3: serve Command Is a Stub

**Current state:** The `Serve` arm in `main.rs` logs a warning and returns. No HTTP server is
started, no store is opened.

**Solution:** Implement a `run_serve(bind: String, data_dir: PathBuf) -> anyhow::Result<()>`
function in `src/daemon/mod.rs` (or a new `src/serve.rs`). This function:
1. Opens `ChunkStore` at the specified data dir
2. Resolves `VOYAGE_API_KEY`
3. Creates `AppState`
4. Builds the combined router (metrics + dashboard)
5. Binds and serves until SIGINT

No file watcher is started. No metrics recorder installation needed if metrics are not required
for `serve` mode — but since `/metrics` is part of the router, it should install the Prometheus
recorder.
[VERIFIED: reading src/main.rs Serve arm and src/daemon/mod.rs run_daemon()]

### Gap 4: SearchEngine Owns Embedder Lifetime

**Current state:** `SearchEngine<'a, E: Embedder>` takes references with lifetimes. For
web handlers, the engine needs to be constructed per-request or stored in AppState.

**Options:**
- Construct `SearchEngine::new(&store, &embedder)` inside each search handler (cheap, no
  allocation — SearchEngine is just two references)
- Store engine in AppState (requires making SearchEngine owned, which requires changes to its
  lifetime generics)

**Recommendation:** Construct `SearchEngine::new(&app_state.store, &app_state.embedder)` inside
the search handler. This is consistent with how it's used in `main.rs` (constructed per
invocation). Zero allocation overhead.
[VERIFIED: reading src/search/engine.rs SearchEngine struct definition]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTML templating | String concatenation, format!() | askama + askama_web | Type-safe, compile-time checked, template inheritance support |
| IntoResponse for templates | Manual impl per struct | askama_web WebTemplate derive | askama_web handles HTML content-type, status codes, rendering errors |
| Query param parsing | Manual URL string parsing | axum `Query<T>` extractor + serde | Built into axum, handles malformed params gracefully |
| Shared state across handlers | Global statics or thread-locals | `axum::extract::State<Arc<AppState>>` | The idiomatic axum pattern, no unsafe required |
| HTML escaping | Manual `<` → `&lt;` replacement | askama auto-escapes by default | HTML auto-escaping is built into askama for all `{{ }}` expressions |

---

## Common Pitfalls

### Pitfall 1: Using Deprecated askama_axum Crate

**What goes wrong:** Adding `askama_axum = "0.4"` to Cargo.toml — crates.io shows it as
`"0.5.0+deprecated"` with a note that integration crates were removed in askama 0.13.
**Why it happens:** Old blog posts and Stack Overflow answers use `askama_axum`.
**How to avoid:** Use `askama_web = { version = "0.15", features = ["axum-0.8"] }`.
[VERIFIED: cargo search askama_axum returns "0.5.0+deprecated"]

### Pitfall 2: Templates Directory Not Found at Compile Time

**What goes wrong:** Compile error — "template file not found" — if `templates/` is not at the
crate root (same directory as `Cargo.toml`).
**Why it happens:** askama resolves template paths relative to `CARGO_MANIFEST_DIR`, not `src/`.
**How to avoid:** Create `templates/` at `/Users/matthewstump/src/local-index/templates/`.
[ASSUMED — based on askama docs description and GitHub examples; consistent across all sources]

### Pitfall 3: SearchEngine Lifetime Conflict with AppState

**What goes wrong:** Trying to store `SearchEngine` in `AppState` causes lifetime errors because
`SearchEngine<'a, E>` borrows from store and embedder.
**Why it happens:** The generic lifetime `'a` cannot outlive the AppState's owned fields.
**How to avoid:** Construct `SearchEngine::new(&store, &embedder)` inside each handler, not in
AppState. This is cheap (no allocation).
[VERIFIED: reading src/search/engine.rs]

### Pitfall 4: HTML Auto-Escaping Breaking Breadcrumb Display

**What goes wrong:** Heading breadcrumbs like `"Goals > Q1 > Details"` are rendered as
`"Goals &gt; Q1 &gt; Details"` by askama's default HTML auto-escaping.
**Why it happens:** Askama escapes `{{ value }}` by default for HTML safety.
**How to avoid:** If breadcrumbs use ` > ` as separator (it does — see `src/pipeline/chunker.rs`
and the types), render as-is — the `>` character IS in the data, not `&gt;`. Askama escapes
HTML special chars, and `>` is a special char. Use `{{ value|safe }}` only for trusted HTML, or
restructure the breadcrumb display to split on ` > ` and rejoin with a styled separator.
[VERIFIED: reading src/search/engine.rs — heading_breadcrumb field value]

### Pitfall 5: VoyageEmbedder Not Sendable for AppState

**What goes wrong:** Compile error if VoyageEmbedder doesn't implement `Send + Sync`.
**Why it happens:** axum requires all state to be `Send + Sync` for multi-threaded tokio.
**How to avoid:** Verify `VoyageEmbedder` is `Send + Sync` (it should be — it's a struct with
`reqwest::Client` which is `Send + Sync`). If not, wrap in `Arc<Mutex<>>` (but avoid if possible).
[ASSUMED — reqwest::Client is Send + Sync per reqwest docs; needs compile-time confirmation]

### Pitfall 6: Serve Command Missing Prometheus Recorder Install

**What goes wrong:** `/metrics` returns empty or panics if the PrometheusBuilder recorder is
not installed before the route is served.
**Why it happens:** `run_daemon()` calls `metrics::setup_metrics()` — `run_serve()` must too.
**How to avoid:** Call `metrics::setup_metrics()` in `run_serve()` before building the router,
just as in `run_daemon()`.
[VERIFIED: reading src/daemon/mod.rs run_daemon()]

---

## Code Examples

### Installing askama_web (Cargo.toml addition)

```toml
# Templates
askama = "0.15"
askama_web = { version = "0.15", features = ["axum-0.8"] }
```

[VERIFIED: cargo search askama = 0.15.6, askama_web = 0.15.2]

### Template Struct Pattern

```rust
// Source: askama-rs/askama_web GitHub (verified 2026-04-10)
use askama::Template;
use askama_web::WebTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "search.html")]
pub struct SearchTemplate {
    pub query: Option<String>,
    pub mode: String,
    pub results: Vec<SearchResultView>,
    pub result_count: usize,
    pub active_nav: &'static str,
}
```

### Handler Returning Template

```rust
// [ASSUMED — standard axum 0.8 handler pattern]
use axum::extract::{Query, State};
use std::sync::Arc;

async fn search_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<SearchTemplate, AppError> {
    let query = params.q.unwrap_or_default();
    if query.is_empty() {
        return Ok(SearchTemplate {
            query: None,
            mode: "hybrid".to_string(),
            results: vec![],
            result_count: 0,
            active_nav: "search",
        });
    }
    let engine = SearchEngine::new(&state.store, &state.embedder);
    let opts = SearchOptions { query: query.clone(), limit: 20, mode: SearchMode::Hybrid, .. };
    let response = engine.search(&opts).await.map_err(AppError::Search)?;
    Ok(SearchTemplate {
        query: Some(query),
        mode: "hybrid".to_string(),
        results: response.results.into_iter().map(SearchResultView::from).collect(),
        result_count: response.total,
        active_nav: "search",
    })
}
```

### Dashboard Router

```rust
// [ASSUMED — standard axum Router pattern]
pub fn dashboard_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(search_handler))
        .route("/search", get(search_handler))
        .route("/index", get(index_handler))
        .route("/status", get(status_handler))
        .route("/settings", get(settings_handler))
        .with_state(state)
}
```

### Count Chunks Per File (new ChunkStore method needed)

```rust
// Add to src/pipeline/store.rs [VERIFIED: method does not exist, design is new]
pub async fn count_chunks_per_file(&self) -> Result<Vec<(String, usize)>, LocalIndexError> {
    let batches: Vec<RecordBatch> = self
        .table
        .query()
        .select(Select::Columns(vec!["file_path".to_string()]))
        .execute()
        .await
        .map_err(|e| LocalIndexError::Database(e.to_string()))?
        .try_collect()
        .await
        .map_err(|e| LocalIndexError::Database(e.to_string()))?;

    let mut counts: HashMap<String, usize> = HashMap::new();
    for batch in &batches {
        let col = get_string_column(batch, "file_path");
        if let Some(arr) = col {
            for i in 0..arr.len() {
                if !arr.is_null(i) {
                    *counts.entry(arr.value(i).to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut result: Vec<(String, usize)> = counts.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `askama_axum` crate | `askama_web` with `axum-0.8` feature | askama 0.13 | Must use new crate; old crate is deprecated on crates.io |
| Manual IntoResponse impl | `#[derive(WebTemplate)]` | askama_web 0.14+ | One derive, no boilerplate |
| axum 0.7 `Router::new().route()` syntax | axum 0.8 — same syntax, but `serve()` API changed | axum 0.8 | Already using correct API (see daemon/mod.rs) |

**Deprecated/outdated:**
- `askama_axum` crate: deprecated since askama 0.13; crates.io shows `0.5.0+deprecated`
- `askama_derive_axum` (joshka's): a community workaround; superseded by `askama_web`

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Templates directory is `templates/` at crate root (CARGO_MANIFEST_DIR) | Architecture Patterns | If wrong: compile error "template not found"; easy to fix by checking askama docs |
| A2 | askama block/extends syntax is `{% extends %}` / `{% block %}` (Jinja2-style) | Code Examples | If wrong: template compile error; askama docs would show correct syntax |
| A3 | `VoyageEmbedder` is `Send + Sync` | Common Pitfalls | If wrong: axum State extractor fails to compile; fix by wrapping in Arc<Mutex<>> |
| A4 | `SearchEngine::new()` inside handler is zero-overhead (no allocation) | Architecture Patterns | If wrong: minor perf impact only; acceptable for operator tool |
| A5 | axum `Query<T>` returns empty struct (not error) when query params are absent | Code Examples | If wrong: handler errors on GET /; fix by using `Option<T>` in params struct |
| A6 | `>` in breadcrumb strings will be HTML-escaped by askama to `&gt;` | Common Pitfalls | If wrong: XSS risk (low — local tool); display issue (likely) |

**All critical library version claims are VERIFIED via `cargo search` (2026-04-10).**

---

## Open Questions

1. **Timestamp display in index browser and status view**
   - What we know: no `indexed_at` column exists in the LanceDB schema
   - What's unclear: user expectation — will showing "—" be acceptable for v1?
   - Recommendation: Show "—" (em-dash) with a tooltip or footnote "Available in a future version". This is honest and avoids schema migration risk. The status CLI already returns `null`.

2. **serve vs daemon: should `/metrics` endpoint be included in `serve`?**
   - What we know: `run_daemon()` installs PrometheusBuilder and serves `/metrics`. `serve` uses the same HTTP stack.
   - What's unclear: whether an operator using `serve` wants `/metrics`.
   - Recommendation: Include `/metrics` in `serve` — it costs nothing and is consistent with daemon behavior.

3. **Embedding model ID for WEB-05**
   - What we know: `embedding_model` is stored per-chunk. Can be read from any row.
   - What's unclear: what to show if store is empty (no chunks).
   - Recommendation: Show "voyage-3.5" as the configured default, regardless of stored data. This is what `VoyageEmbedder::model_id()` returns.

---

## Environment Availability

Step 2.6: SKIPPED — Phase 5 is a pure Rust code/template addition. All dependencies are Cargo
packages; no external tools, services, CLIs, or databases beyond what already runs in the project
are required. The LanceDB database is embedded. No Docker, no separate processes.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in (`#[test]`, `#[tokio::test]`) |
| Config file | None — standard cargo test |
| Quick run command | `cargo test --lib 2>&1` |
| Full suite command | `cargo test 2>&1` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-05 | `serve` command starts HTTP server | integration | `cargo test serve_command 2>&1` | Wave 0 |
| WEB-01 | HTTP server binds on configured port | integration | `cargo test http_server_binds 2>&1` | Wave 0 |
| WEB-02 | Search handler returns results HTML | unit | `cargo test --lib web::handlers::test_search 2>&1` | Wave 0 |
| WEB-03 | Index handler returns file list HTML | unit | `cargo test --lib web::handlers::test_index 2>&1` | Wave 0 |
| WEB-04 | Status handler returns status HTML | unit | `cargo test --lib web::handlers::test_status 2>&1` | Wave 0 |
| WEB-05 | Status page shows embedding model ID | unit | `cargo test --lib web::handlers::test_status_model 2>&1` | Wave 0 |
| WEB-06 | Settings handler shows config values | unit | `cargo test --lib web::handlers::test_settings 2>&1` | Wave 0 |

The existing axum test pattern (`tower::ServiceExt::oneshot`) from `src/daemon/http.rs` is the
right model for all handler tests.

### Sampling Rate

- **Per task commit:** `cargo test --lib 2>&1`
- **Per wave merge:** `cargo test 2>&1`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `src/web/mod.rs` — new module with handlers and context types
- [ ] `templates/base.html` — layout shell (CSS, nav, main wrapper)
- [ ] `templates/search.html` — search form + results
- [ ] `templates/index.html` — file table
- [ ] `templates/status.html` — status key-value
- [ ] `templates/settings.html` — settings key-value
- [ ] `templates/error.html` — 500 error page
- [ ] `src/web/handlers.rs` with `#[cfg(test)]` blocks for each handler

---

## Security Domain

> `security_enforcement` not explicitly false in config.json — including section.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | Dashboard binds to 127.0.0.1 only; no authentication required (per requirements: "WebUI authentication: Local daemon on 127.0.0.1; auth adds complexity with no security benefit") |
| V3 Session Management | No | No sessions — stateless GET-only dashboard |
| V4 Access Control | No | Local-only, no multi-user context |
| V5 Input Validation | Yes | Search query and mode param deserialized via serde; mode is validated against known values; askama HTML auto-escapes all output |
| V6 Cryptography | No | No cryptographic operations in dashboard layer |

### Known Threat Patterns for axum/HTML dashboard

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| XSS via search query reflected in results | Spoofing | askama auto-escapes `{{ }}` expressions by default — do NOT use `|safe` filter on user input |
| API key exposure via settings page | Information Disclosure | Settings view shows credential SOURCE not VALUE — never pass `VOYAGE_API_KEY` value to template context |
| Path traversal via `q=` param | Tampering | Search query goes to LanceDB FTS/vector search, not filesystem; no path traversal risk |

---

## Sources

### Primary (HIGH confidence)

- `cargo search askama` (2026-04-10) — version 0.15.6 confirmed
- `cargo search askama_web` (2026-04-10) — version 0.15.2 confirmed; `axum-0.8` feature confirmed
- `cargo search askama_axum` (2026-04-10) — `0.5.0+deprecated` confirmed
- `/Users/matthewstump/src/local-index/src/daemon/http.rs` — existing router structure
- `/Users/matthewstump/src/local-index/src/pipeline/store.rs` — schema, available methods
- `/Users/matthewstump/src/local-index/src/search/engine.rs` — SearchEngine lifetime, API
- `/Users/matthewstump/src/local-index/src/cli.rs` — Serve command definition
- `/Users/matthewstump/src/local-index/src/main.rs` — Serve stub, run_daemon pattern
- `/Users/matthewstump/src/local-index/Cargo.toml` — current dependencies
- `https://github.com/askama-rs/askama_web` — WebTemplate derive pattern, axum-0.8 feature

### Secondary (MEDIUM confidence)

- WebSearch: "askama 0.15 axum IntoResponse 2025" — confirmed askama_axum deprecated, askama_web recommended
- `https://github.com/askama-rs/askama/tree/main/examples` — axum-app example exists using axum 0.8.1

### Tertiary (LOW confidence)

- Template inheritance syntax (`{% extends %}`, `{% block %}`) inferred from askama documentation description of "Jinja-like"; not verified with `cargo expand` output

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — version numbers verified via cargo search
- Architecture: MEDIUM — handler/router patterns based on existing codebase patterns plus axum 0.8 conventions
- Pitfalls: HIGH — askama_axum deprecation verified; SearchEngine lifetime verified from source; others are MEDIUM (inferred patterns)
- Gap analysis: HIGH — schema gaps verified by reading store.rs; stub verified by reading main.rs

**Research date:** 2026-04-10
**Valid until:** 2026-05-10 (askama/axum are stable; 30 days safe)
