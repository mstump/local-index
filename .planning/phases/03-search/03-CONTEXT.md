# Phase 3: Search - Context

**Gathered:** 2026-04-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire the `search` command to perform semantic (vector ANN), full-text (BM25), and hybrid (RRF) queries against the LanceDB index and return structured results. The CLI is already fully defined from Phase 1 — no new flags or subcommands needed. This phase is pure backend implementation: query execution, result assembly, filtering, and output formatting.

Scope includes: semantic search (vector ANN via LanceDB), FTS (BM25 via LanceDB built-in FTS), hybrid fusion (RRF), `--path-filter`, `--tag-filter`, `--context N`, `--format json|pretty`, score thresholding.

Out of scope: daemon mode, file watching, web dashboard, HyDE (hypothetical document expansion), LLM reranking.

</domain>

<decisions>
## Implementation Decisions

### FTS Backend
- **D-01:** Use LanceDB's built-in FTS for BM25 full-text search. Single table, no extra files, no tantivy dependency. Trade-off accepted: less control over tokenization, newer API — if a show-stopper bug is hit during implementation, switch to tantivy without changing the search interface.

### JSON Output Shape
- **D-02:** The JSON output is a **wrapped object**, not a bare array:
  ```json
  {
    "query": "<original query string>",
    "mode": "hybrid",
    "total": N,
    "results": [...]
  }
  ```
  This allows Claude Code callers to verify the query was interpreted correctly and check total result count without re-parsing the array.

- **D-03:** Each result object contains exactly: `chunk_text`, `file_path` (vault-relative), `heading_breadcrumb`, `similarity_score` (fused RRF score), `semantic_score`, `fts_score`, `line_range` (`{"start": N, "end": N}`), `frontmatter` (`{"tags": [...], "aliases": [...], ...}`).

### Hybrid Score Reporting
- **D-04:** In hybrid mode, report **all three scores**: `similarity_score` (fused RRF, 0.0–1.0), `semantic_score` (raw vector similarity, 0.0–1.0), `fts_score` (raw BM25 normalized score, 0.0–1.0). In semantic-only mode, `fts_score` is omitted (or null). In FTS-only mode, `semantic_score` is omitted (or null). This gives callers maximum transparency without breaking simple consumers that only read `similarity_score`.

### Pretty Output Format
- **D-05:** `--format pretty` renders **snippet blocks** (not a table):
  ```
  [1] notes/projects/foo.md — ## Goals > ### Q1  (score: 0.83)
  ════════════════════════════════════════
  ## Goals > ### Q1
  We aim to ship the semantic search MVP...
  [truncated to ~3 lines]

  [2] notes/projects/bar.md — ## Architecture  (score: 0.71)
  ...
  ```
  Uses `═` separator lines. Chunk text truncated at ~200 chars with `[truncated]` marker if longer. File path shown vault-relative. No terminal table — avoids column alignment issues on narrow terminals.

### Hybrid Fusion (RRF)
- **D-06:** Claude's discretion on RRF k constant (standard: k=60). Normalize both semantic and FTS scores to 0.0–1.0 before fusion. Return top-N after fusion.

### Context Chunks (`--context N`)
- **D-07:** Claude's discretion on representation. Suggested: include context chunks in the `results` array with an additional `is_context: true` field and `context_for_index` pointing to the matching result's index. This keeps the array flat and easy to iterate.

### Query Embedding
- **D-08:** For semantic and hybrid modes, the query string is embedded directly using `VoyageEmbedder` (same embedder used for indexing). No HyDE or query expansion in v1.

### Claude's Discretion
- LanceDB FTS index creation API (create_index call, tokenizer options — use defaults)
- RRF k constant (standard: 60)
- Exact BM25 score normalization approach
- Context chunk representation details
- Error messages for "index not found" (when LanceDB table is empty or doesn't exist)
- indicatif/progress behavior during search (probably none — search is fast)

</decisions>

<specifics>
## Specific Ideas

- `ChunkStore` from Phase 2 already holds the LanceDB connection and table — search should reuse it rather than opening a new connection. Consider adding search methods directly to `ChunkStore` or creating a `ChunkSearcher` that wraps the same connection.
- The `--min-score` filter applies to `similarity_score` (the fused score in hybrid mode, the raw score in single-mode).
- Empty results should return a valid wrapped object with `"total": 0` and `"results": []`, not an error.
- `--path-filter` filters by vault-relative file_path prefix (e.g., `--path-filter notes/projects/` matches all files under that directory).
- `--tag-filter` filters by frontmatter tags array membership.
- TTY detection: `--format json` always outputs JSON regardless of TTY. `--format pretty` outputs snippet blocks. No auto-detection needed for search (unlike index command) — the caller sets format explicitly.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` §SRCH-01..10 — All search requirements: modes, flags, output format, filters, context
- `.planning/REQUIREMENTS.md` §CLI-03 — `local-index search "<query>"` command spec

### Project constraints
- `.planning/PROJECT.md` §Constraints — Rust only, single binary, embedded LanceDB
- `.planning/PROJECT.md` §Context — Primary consumer is Claude Code (JSON output must be machine-parseable)

### Phase 2 output (inputs to this phase)
- `.planning/phases/02-storage-embedding-pipeline/02-02-SUMMARY.md` — ChunkStore API: open, store_chunks, get_hashes_for_file, delete_chunks_for_file; table schema (10 columns including vector)
- `.planning/phases/02-storage-embedding-pipeline/02-01-SUMMARY.md` — VoyageEmbedder API: embed(), model_id(), dimensions()
- `src/pipeline/store.rs` — ChunkStore struct and LanceDB connection pattern to reuse
- `src/pipeline/embedder.rs` — VoyageEmbedder and Embedder trait

### CLI definition (already complete)
- `src/cli.rs` — Search command, SearchMode enum (Semantic/Fts/Hybrid), OutputFormat enum (Json/Pretty) — all flags already defined

### Tech stack guidance
- `CLAUDE.md` §Recommended Stack — LanceDB FTS availability note, vector search, tantivy fallback rationale
- `CLAUDE.md` §Key Risk Areas — LanceDB Rust API FTS exposure (now confirmed available)

### Prior art
- ROADMAP.md §Phase 3 Search design notes — qmd analysis (reference only; CLI uses `--mode` flag, not subcommand split)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/pipeline/store.rs` — `ChunkStore { db: Connection, table: Table }` holds the LanceDB connection. Add search methods here or create a companion `ChunkSearcher` wrapping the same connection.
- `src/pipeline/embedder.rs` — `VoyageEmbedder` for query embedding (semantic/hybrid modes).
- `src/credentials.rs` — `resolve_voyage_key()` needed to construct embedder for semantic queries.
- `src/cli.rs` — `SearchMode` and `OutputFormat` enums already defined; `Command::Search` struct with all fields.
- `src/error.rs` — `LocalIndexError::Database` for LanceDB search errors.

### Established Patterns
- Per-result graceful error handling: if a single result fails to decode, warn and skip — never abort the search.
- `tracing` spans for search latency instrumentation.
- `anyhow` in `main.rs`, `thiserror` for library error types.
- `#[tokio::main]` already on `main()` from Phase 2 — all async search code works directly.

### Integration Points
- `src/main.rs` `Commands::Search` arm — currently logs "search command not yet implemented". Phase 3 replaces this with the full search pipeline.
- `--data-dir` global flag provides the LanceDB database path (same as used by index command).

</code_context>

<deferred>
## Deferred Ideas

- HyDE (hypothetical document expansion) — mentioned in qmd analysis; deferred to v2. Requires LLM call per query.
- LLM reranking — qmd uses local GGUF models; not our deployment model. Deferred.
- Query expansion / synonyms — nice-to-have for v2.
- Streaming results — all results returned at once in v1; streaming deferred.
- Tantivy sidecar index — accepted LanceDB FTS as the approach; tantivy is the fallback if LanceDB FTS hits a show-stopper bug.

</deferred>

---

*Phase: 03-search*
*Context gathered: 2026-04-10*
