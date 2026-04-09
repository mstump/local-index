# Research Summary: local-index

**Domain:** File-indexing daemon with vector/full-text search
**Researched:** 2026-04-08
**Overall confidence:** MEDIUM (web verification tools unavailable; versions unverified)

## Executive Summary

local-index is a Rust daemon that watches a markdown vault, chunks files by heading, embeds chunks via the Anthropic API, stores them in embedded LanceDB, and exposes search via CLI and web dashboard. The Rust ecosystem has mature crates for every layer of this system, but two areas carry risk: LanceDB's Rust API completeness (the Python API is more feature-rich) and the Anthropic embeddings endpoint specifics (model name, API shape).

The recommended stack is tokio + axum + notify + lancedb + tantivy + pulldown-cmark + metrics-rs. This is a conventional Rust daemon stack with no exotic dependencies. The primary architectural challenge is coordinating the file watcher, embedding pipeline, and search index in a single async process without blocking the web server or CLI.

The project benefits from a phased approach: get file watching and chunking working first (no external dependencies), then add embedding and storage, then search, then the web dashboard. Each phase is independently testable. The Anthropic API integration and LanceDB storage are the highest-risk components and should be prototyped early.

A key decision point is whether LanceDB's Rust crate exposes full-text search. If yes, use it for both vector and text search. If no, run tantivy as a sidecar index for FTS alongside LanceDB for vector search. This must be validated in Phase 1.

## Key Findings

**Stack:** tokio + axum + notify + lancedb + tantivy + pulldown-cmark + metrics-rs + clap + tracing. All standard Rust 2025 crates.
**Architecture:** Single-process async daemon with background file watcher, embedding pipeline, dual search indexes, and HTTP server on one tokio runtime.
**Critical pitfall:** LanceDB Rust API may not expose FTS -- if true, need tantivy alongside, doubling index management complexity.

## Implications for Roadmap

Based on research, suggested phase structure:

1. **Foundation + File Processing** - Establish project structure, CLI skeleton, markdown chunking
   - Addresses: CLI framework, markdown parsing, file watching
   - Avoids: External dependency risk (no API calls yet)
   - Validates: pulldown-cmark heading extraction works for Obsidian markdown

2. **Storage + Embedding** - LanceDB integration and Anthropic API client
   - Addresses: Vector storage, embedding pipeline, credential resolution
   - Avoids: Building on unvalidated storage layer
   - Validates: LanceDB Rust API capabilities (especially FTS), Anthropic embedding model/endpoint

3. **Search** - Vector search, full-text search, hybrid ranking
   - Addresses: Core search functionality, CLI search command
   - Avoids: Premature optimization of search before data pipeline works
   - Validates: Search quality, tantivy integration (if needed)

4. **Daemon Mode + Observability** - File watching daemon, metrics, incremental updates
   - Addresses: Real-time indexing, Prometheus metrics, HDR histograms
   - Avoids: Running daemon before search is validated

5. **Web Dashboard + Polish** - axum server, askama templates, htmx interactivity
   - Addresses: WebUI requirements, status endpoints, settings view
   - Avoids: Building UI before core functionality is solid

**Phase ordering rationale:**
- Phase 1 has zero external dependencies (no API calls, no DB) -- fast to validate
- Phase 2 tackles the two highest-risk integrations (LanceDB, Anthropic API) early
- Phase 3 depends on Phase 2 (need stored embeddings to search)
- Phase 4 depends on Phase 1+2 (daemon wraps file watching + embedding pipeline)
- Phase 5 is purely additive (dashboard reads existing data/metrics)

**Research flags for phases:**
- Phase 2: NEEDS deeper research -- LanceDB Rust API capabilities, Anthropic embedding model name and API shape
- Phase 3: MAY need research -- hybrid search ranking strategies if using tantivy + lancedb together
- Phase 5: Standard patterns, unlikely to need research

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM | Crate choices are solid; versions unverified against crates.io |
| Features | HIGH | Requirements are well-defined in PROJECT.md |
| Architecture | HIGH | Standard async daemon pattern, well-understood |
| Pitfalls | MEDIUM | LanceDB Rust maturity and Anthropic API specifics are key unknowns |

## Gaps to Address

- LanceDB Rust crate: does it expose FTS? What Arrow version does it pin? What's the actual current version?
- Anthropic embeddings: what is the current model name? Is it `voyage-3`, a native Anthropic model, or something else? What's the exact API endpoint?
- `~/.claude/` credential format: what files exist, what's the JSON structure? Needs hands-on investigation.
- axum version: is 0.8 released or still 0.7? Minor but affects API surface.
- All crate versions need verification via `cargo add` before implementation.
