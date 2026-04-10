<!-- GSD:project-start source:PROJECT.md -->
## Project

**local-index**

A Rust daemon that watches a directory tree (initially an Obsidian vault), chunks markdown files by heading, embeds each chunk via the Anthropic API, and stores everything in an embedded LanceDB database. Exposes full-text and semantic search via CLI, a Claude Code skill interface, and a web dashboard — enabling Claude to reason over your notes without resorting to grep.

**Core Value:** Fast, accurate semantic search over a local markdown vault that Claude can query as a skill without any manual intervention.

### Constraints

- **Tech stack**: Rust only — no Node/Python helpers
- **Embeddings**: Anthropic API only in v1 (no local models, no OpenAI)
- **Database**: LanceDB embedded — no external database process
- **CLI framework**: `clap` with derive macros
- **Logging**: `tracing` crate (no `log` crate directly)
- **Metrics**: Prometheus-compatible `/metrics` endpoint; HDR histograms for latency
- **Deployment**: Single binary, runs on macOS (primary), Linux (secondary)
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## Recommended Stack

### Async Runtime

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tokio | ^1.40 | Async runtime | The Rust async runtime. axum requires it, reqwest requires it, notify has async support via it. No reason to consider anything else. | HIGH |
| tokio (features) | `full` | All tokio features | Need fs, net, time, sync, macros, rt-multi-thread. `full` is simpler than cherry-picking for a daemon. | HIGH |

### File Watching

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| notify | ^7.0 | Cross-platform file watching | The standard Rust file-watching crate. Uses FSEvents on macOS, inotify on Linux. Mature, well-maintained, async-compatible via `notify-debouncer-full`. | MEDIUM (verify version) |
| notify-debouncer-full | ^0.4 | Debounced file events | Raw notify fires duplicate/rapid events. The debouncer coalesces events, provides rename tracking, and reduces noise. Essential for a daemon that re-indexes on change. | MEDIUM (verify version) |

- `hotwatch` -- thin wrapper around `notify` that adds nothing useful and lags behind notify releases. Use notify directly.
- `watchexec-events` -- designed for process-restarting use cases, not embedding in a daemon.

### Embedding Storage (Vector DB)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| lancedb | ^0.13 | Embedded vector database | Project requirement. Rust-native embedded mode, no server process. Supports vector search (IVF_PQ, flat), filtering, and schema evolution. Async API via Arrow. | LOW (verify version -- this crate evolves rapidly) |
| arrow | ^53.0 | Arrow data types | LanceDB uses Apache Arrow for its data model. You'll need `arrow` for constructing RecordBatches to insert. Version must match what lancedb depends on. | LOW (must match lancedb's dep) |

- The Rust crate is the native implementation (not a wrapper around Python). It is async-first.
- Schema is defined via Arrow schema types. Each "table" maps to a Lance dataset on disk.
- Vector search returns results with distance scores. Use cosine distance for Anthropic embeddings.
- LanceDB has a built-in full-text search (FTS) capability based on tantivy internally. Verify whether the Rust crate exposes FTS or if it's Python-only -- this is a key risk area. If FTS is not exposed in the Rust crate, use tantivy directly alongside it.

### Full-Text Search

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tantivy | ^0.22 | Full-text search engine | Rust-native, fast, well-maintained. If LanceDB's Rust API does not expose FTS, use tantivy as a sidecar index. Even if LanceDB has FTS, tantivy gives more control over tokenization and ranking. | MEDIUM (verify version) |

### Anthropic API Client

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| reqwest | ^0.12 | HTTP client for Anthropic API | Build a thin wrapper around reqwest rather than depending on a third-party Anthropic crate. The embeddings API is a single POST endpoint. A dedicated client crate adds dependency risk for minimal value. | HIGH (on approach; verify version) |
| serde | ^1.0 | JSON serialization | Serialize/deserialize Anthropic API request/response types. | HIGH |
| serde_json | ^1.0 | JSON processing | Parse API responses. | HIGH |

- `misanthropy` -- small community crate, unclear maintenance. For a single API endpoint (embeddings), a bespoke reqwest wrapper is less risky than a third-party dependency that may lag behind API changes.
- `async-anthropic` -- same concern. The Anthropic embeddings endpoint is simple: POST with model + input text, get back a vector. Custom 30-line client is better than an external dep.

### Web Framework

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| axum | ^0.8 | Web framework for dashboard + metrics | The standard choice for new Rust web projects in 2025. Built on tokio + tower + hyper. Excellent ergonomics, composable middleware via tower, first-class async. | MEDIUM (verify 0.8 is released; may still be 0.7) |
| tower | ^0.5 | Middleware layer | Timeout, rate-limiting, compression middleware. axum is built on tower. | MEDIUM (verify version) |
| tower-http | ^0.6 | HTTP-specific middleware | CORS, static file serving (for dashboard assets), compression, tracing. | MEDIUM (verify version) |
| askama | ^0.13 | HTML templating | Compile-time templates for the web dashboard. Type-safe, fast, Jinja2-like syntax. Better than runtime templates (tera) for a Rust project -- catches errors at compile time. | MEDIUM (verify version) |

- `actix-web` -- still a fine framework but the ecosystem has consolidated around axum for new projects. axum's tower compatibility gives better middleware reuse.
- `warp` -- maintenance has slowed. axum supersedes it.
- `tera` -- runtime templates. askama's compile-time checking is worth the slightly less flexible syntax.

### CLI Framework

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| clap | ^4.5 | CLI argument parsing | Project requirement (clap + derive). Industry standard. | HIGH (on crate; verify minor version) |

### Logging / Tracing

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tracing | ^0.1 | Structured logging + spans | Project requirement. The standard Rust instrumentation crate. Structured logging with spans for request/operation tracing. | HIGH |
| tracing-subscriber | ^0.3 | Log output formatting | Provides `fmt` subscriber for console output, `EnvFilter` for RUST_LOG-based filtering. | HIGH |

### Metrics / Observability

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| metrics | ^0.24 | Metrics facade | The `metrics` crate is a facade (like `log` for logging). Decouple metric recording from export. Use `metrics::histogram!()` macros throughout code. | MEDIUM (verify version) |
| metrics-exporter-prometheus | ^0.16 | Prometheus export | Renders `/metrics` endpoint in Prometheus exposition format. Plugs into the `metrics` facade. | MEDIUM (verify version) |

- `prometheus` crate (the one from tikv) -- older API style, requires manual `Registry` management, verbose. The `metrics` facade + exporter approach is more ergonomic and idiomatic in 2025.
- `opentelemetry` -- overkill for this project. OTel is for distributed tracing across services. A single-binary daemon only needs Prometheus metrics + tracing logs.

### Markdown Parsing

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| pulldown-cmark | ^0.12 | Markdown parsing | The standard Rust markdown parser. Pull-based (streaming), fast, CommonMark-compliant. Perfect for heading extraction -- iterate events, track heading starts/ends, collect text between headings. | MEDIUM (verify version) |

- `comrak` -- GFM-compatible but heavier. It builds an AST, which is more allocation-heavy than pulldown-cmark's pull parser. We only need to split by headings, not render to HTML. pulldown-cmark's streaming model is ideal.
- `markdown` -- minimal crate, not as well maintained.

### Configuration

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| dotenvy | ^0.15 | .env file loading | Fork of `dotenv` (which is unmaintained). Loads `.env` into process environment before clap parses. | MEDIUM (verify version) |

- `dotenv` -- unmaintained since 2021. `dotenvy` is the maintained fork.
- `config` (config-rs) -- overkill. The project uses clap for all config with env var fallbacks. Adding config-rs means managing two config systems. Keep it simple: dotenvy loads .env, clap reads env vars via `#[arg(env = "...")]`.

### Credential Parsing

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| serde_json | ^1.0 | Parse ~/.claude/ credentials | The Claude credential store is JSON. serde_json is already a dependency for the Anthropic API client. No additional crate needed. | MEDIUM (on format; verify actual ~/.claude/ file format) |
| dirs | ^6.0 | Home directory resolution | Cross-platform `~` expansion. `dirs::home_dir()` for finding `~/.claude/`. | MEDIUM (verify version) |

### Serialization / Data

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| serde | ^1.0 | Serialization framework | Used everywhere: API types, config, JSON output, credentials. | HIGH |
| serde_json | ^1.0 | JSON | API communication, CLI output, credential parsing. | HIGH |
| chrono | ^0.4 | Date/time | Timestamps for last-indexed, file modification times. | HIGH |
| uuid | ^1.0 | Unique IDs | Chunk IDs for deduplication and stable references. | HIGH |

### Error Handling

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| anyhow | ^1.0 | Application error handling | For the binary/application layer. Ergonomic error chaining with context. | HIGH |
| thiserror | ^2.0 | Library error types | For defining structured error enums in library code (API errors, DB errors, parse errors). | MEDIUM (verify if 2.0 is released; may be 1.x) |

### Testing

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tokio (test) | -- | Async test runtime | `#[tokio::test]` for async tests. Already a dep. | HIGH |
| tempfile | ^3.0 | Temporary directories for tests | Create temp dirs for file-watching tests, temp LanceDB databases. | HIGH |
| wiremock | ^0.6 | HTTP mocking | Mock Anthropic API responses in tests. Async-native, works with tokio. | MEDIUM (verify version) |

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| File watching | notify + debouncer | hotwatch | Thin wrapper, lags behind notify |
| Vector DB | lancedb (embedded) | qdrant | Requires separate server process |
| Anthropic client | reqwest (custom) | misanthropy | Maintenance risk for a simple API |
| Web framework | axum | actix-web | Ecosystem consolidated on axum |
| Templates | askama | tera | Runtime vs compile-time checking |
| Metrics | metrics + prometheus exporter | prometheus crate | Verbose API, older pattern |
| Markdown | pulldown-cmark | comrak | Heavier AST model; we only need streaming |
| Config | dotenvy + clap env | config-rs | Two config systems is unnecessary |
| Full-text search | tantivy (fallback) | meilisearch | Embedded vs server |

## Cargo.toml Sketch

# Async runtime

# File watching

# Vector storage

# Full-text search (if lancedb FTS not available in Rust)

# HTTP client (Anthropic API)

# Web framework

# CLI

# Logging

# Metrics

# Markdown

# Serialization

# Config & environment

# Utilities

## Key Risk Areas

## Sources

- notify crate: <https://github.com/notify-rs/notify>
- lancedb Rust: <https://github.com/lancedb/lancedb> (check /rust/ directory)
- axum: <https://github.com/tokio-rs/axum>
- tantivy: <https://github.com/quickwit-oss/tantivy>
- pulldown-cmark: <https://github.com/raphlinus/pulldown-cmark>
- metrics: <https://github.com/metrics-rs/metrics>
- askama: <https://github.com/djc/askama>
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:

- `/gsd:quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd:debug` for investigation and bug fixing
- `/gsd:execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd:profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
