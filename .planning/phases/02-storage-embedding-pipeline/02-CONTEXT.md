# Phase 2: Storage & Embedding Pipeline - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Embed all chunks produced by the Phase 1 pipeline via Voyage AI, store them in embedded LanceDB alongside full chunk metadata, and support incremental re-indexing by skipping chunks whose content hash is unchanged. Includes credential resolution, exponential backoff on API errors, and progress reporting. Search and daemon mode are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Embedding Provider
- **D-01:** Voyage AI is the sole embedding provider in Phase 2. Google Gemini is deferred.
- **D-02:** Embedding is behind an `Embedder` trait so future providers (Gemini, OpenAI, local models) can be added without changing the pipeline.
- **D-03:** The concrete implementation is `VoyageEmbedder`.

### Credentials
- **D-04:** Credential resolution for Voyage AI checks `VOYAGE_API_KEY` env var only. There is no `~/.claude/` fallback — that was Anthropic-specific and does not apply here.
- **D-05:** Startup fails with a clear, actionable error message if `VOYAGE_API_KEY` is not set (CRED-03).

### index Command Output
- **D-06:** Output mode is auto-detected via TTY check on stdout (`std::io::IsTerminal`).
- **D-07:** Interactive mode (TTY): `indicatif` progress bar showing files processed / chunks embedded / errors, followed by a human-readable summary line (e.g., "Indexed 42 files • 312 chunks embedded • 8 skipped • 0 errors").
- **D-08:** Agent/pipe mode (non-TTY): progress lines to stderr, one JSON summary object to stdout on completion: `{"files_indexed": N, "chunks_embedded": N, "chunks_skipped": N, "errors": N}`.

### Content Hash (Incremental Indexing)
- **D-09:** SHA-256 is computed over the concatenation of: `body` text + `heading_breadcrumb` + serialized `frontmatter`. Any change to any of these fields invalidates the hash and triggers re-embedding (INDX-04).

### Claude's Discretion
- Voyage AI model name and embedding dimensions (researcher to confirm from Voyage AI docs)
- LanceDB table schema column layout (follow INDX-05 requirements)
- Exact exponential backoff parameters (base delay, jitter range, max retries) — follow INDX-07
- indicatif bar style and tick rate

</decisions>

<specifics>
## Specific Ideas

- The `Embedder` trait should be designed so adding a new provider in a future phase requires only a new struct — no changes to the pipeline itself.
- TTY detection should match the pattern established by tools like `cargo build`: automatic, no explicit flag needed.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` §CRED-01..03 — Credential resolution, Embedder trait, startup error behavior
- `.planning/REQUIREMENTS.md` §INDX-04..08 — Content hash, LanceDB schema, model mismatch guard, backoff, progress reporting

### Project constraints
- `.planning/PROJECT.md` §Constraints — Rust only, single binary, embedded LanceDB, no external database process
- `.planning/PROJECT.md` §Context — LanceDB embedded, primary consumer is Claude Code (JSON output must be machine-parseable)

### Phase 1 output (input to this phase)
- `.planning/phases/01-foundation-file-processing/01-03-SUMMARY.md` — What Phase 1 delivered; `Chunk` struct shape and pipeline API

### Tech stack guidance
- `CLAUDE.md` §Recommended Stack — LanceDB, arrow, reqwest, serde, tokio versions and rationale; risk flags on LanceDB FTS and embedding model names

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/types.rs` — `Chunk` struct (body, file_path, heading_breadcrumb, heading_level, line_start, line_end, frontmatter) is the input to this phase's embedding pipeline. `Frontmatter` struct with serde derives is ready for hash serialization.
- `src/error.rs` — `LocalIndexError` enum needs new variants: `Embedding`, `Database`, `Credential`. Existing `Io` and `Config` variants are reusable.
- `src/pipeline/mod.rs` — Add `embedder` and `store` submodules here alongside existing `walker` and `chunker`.

### Established Patterns
- Per-file graceful error handling: warn via `tracing::warn!` and continue — never abort the walk. Apply same pattern to per-chunk embedding failures.
- `tracing` spans for all operations; `RUST_LOG` / `--log-level` for log control. No direct `log` crate usage.
- `anyhow` at the application layer (main.rs), `thiserror` for library error enums.

### Integration Points
- `src/main.rs` `Commands::Index` arm — currently calls `discover_markdown_files` + `chunk_markdown` + prints JSONL. Phase 2 replaces the JSONL output with the store + embed pipeline and progress reporting.
- `--data-dir` global flag already wired in CLI (Phase 1) — use this as the LanceDB database root.

</code_context>

<deferred>
## Deferred Ideas

- Google Gemini `Embedder` implementation — deferred from Phase 2; add in a future phase when needed.
- `~/.claude/` credential fallback — was scoped for Anthropic credentials; not applicable to Voyage AI. May be revisited if an Anthropic-based provider is added in a future phase.

</deferred>

---

*Phase: 02-storage-embedding-pipeline*
*Context gathered: 2026-04-09*
