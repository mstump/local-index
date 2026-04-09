# Feature Landscape

**Domain:** Local semantic search / personal knowledge base indexing daemon
**Researched:** 2026-04-08
**Confidence:** MEDIUM (training data only -- WebSearch/WebFetch unavailable; no live verification performed)

## Table Stakes

Features users expect. Missing = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Heading-based markdown chunking | Obsidian notes are heading-structured; every competitor chunks by heading. Users expect search to return the relevant *section*, not the whole file. | Low | Split on `^#{1,6}\s`. Include frontmatter as metadata on every chunk from that file. Chunk should carry its heading hierarchy breadcrumb (e.g. `## Foo > ### Bar`). |
| Incremental indexing (content-hash) | Re-embedding an entire vault on every change is unacceptable (cost + latency). Users expect near-instant updates when they save a file. | Medium | Hash each chunk's content; skip embedding call if hash unchanged. Store `(file_path, chunk_hash, embedding)`. On file change, re-chunk, diff hashes, embed only new/changed chunks, delete removed ones. |
| File-system watching (daemon mode) | Competitors (Smart Connections, Khoj) watch for changes automatically. Users expect "save file, search finds it seconds later." | Medium | Use `notify` crate. Debounce events (files get multiple writes). Handle create/modify/rename/delete. Rename = delete old + index new (path changes). |
| One-shot full re-index | Needed for first run, CI pipelines, and recovery. Users expect `local-index index` to just work and exit. | Low | Walk directory tree, filter to `.md`, chunk all, embed all (with batching), store. Show progress bar or progress output. |
| Semantic (vector) search | This is the core value proposition. Users expect "search by meaning, not just keywords." | Medium | Embed the query, ANN search in LanceDB. Return top-k results ranked by cosine similarity. LanceDB handles this natively. |
| Full-text search | Users will want to find exact phrases, code snippets, and proper nouns that semantic search fumbles. Every search tool offers this as a fallback. | Medium | LanceDB supports full-text search via Tantivy integration. Use it. Users expect `"exact phrase"` to match literally. |
| Structured JSON output | The primary consumer is Claude Code. Machine-parseable output is non-negotiable. | Low | Every result: `{ chunk_text, file_path, heading_breadcrumb, similarity_score, line_range }`. For CLI human use, add a `--format=pretty` flag that renders a readable table. |
| CLI with subcommands | Users expect `index`, `search`, `status`, `daemon`, `serve`. Standard UX for Rust CLI tools. | Low | Already planned with clap derive. Include `--help` that is genuinely useful (examples, not just flag lists). |
| Configurable result count and threshold | Users need `--limit N` and `--min-score 0.7`. Without these, search is either too noisy or too restrictive. | Low | Sensible defaults: limit=10, no score threshold by default (let user tune). |
| Frontmatter awareness | Obsidian vaults use YAML frontmatter for tags, aliases, dates. Ignoring it means losing critical metadata for filtering and ranking. | Low | Parse frontmatter, store as structured metadata on each chunk. Enable filtering by tags/date in search queries. |
| Graceful error handling for API failures | Anthropic API has rate limits and outages. Users expect the daemon to retry, not crash. | Medium | Exponential backoff with jitter. Queue failed chunks for retry. Log clearly. Never lose indexed data on transient API failure. |

## Differentiators

Features that set this product apart. Not expected, but valued.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Hybrid search (semantic + full-text fusion) | Most local tools do one or the other. Fusing both (e.g., Reciprocal Rank Fusion) gives dramatically better results for mixed queries. This is the single highest-impact differentiator. | Medium | Score from semantic search + score from full-text search, combined via RRF or weighted sum. LanceDB supports hybrid queries natively -- use it. |
| Heading hierarchy as a ranking signal | A match in an `## H2` heading or its immediate content should rank higher than a match buried in an `###### H6`. No competitor does this well. | Low | Store heading depth as metadata. Apply a small boost based on heading level (configurable weight). |
| Context window expansion | Return not just the matching chunk but also the chunk before/after it. Claude Code needs context to reason well. | Low | Store chunk ordering per file. On search hit, optionally include `prev_chunk` and `next_chunk` in results. Controlled by `--context=1` flag. |
| Skill-native Claude Code integration | Most tools bolt on AI integration as an afterthought. This tool is *designed* for Claude Code consumption from day one -- structured JSON, skill files, documented invocation. | Low | Ship `.claude/skills/` files that Claude Code can discover. Include a `search` skill, `reindex` skill, and `status` skill. Each skill invokes the CLI and returns structured JSON. |
| Per-file and per-tag filtering in search | "Search only in files tagged `#project-x`" or "Search only in `daily-notes/`." Users with large vaults need scoping. | Medium | `--path-filter "daily-notes/"`, `--tag-filter "project-x"`. Apply as pre-filter before vector search (LanceDB supports predicate pushdown). |
| Stale index detection and reporting | `local-index status` shows which files are out of date, how many chunks are pending re-embedding, and when the last successful index completed. | Low | Compare file mtimes against last-indexed timestamps. Report stale count in status output and `/metrics`. |
| Chunk overlap / sliding window option | Pure heading-based chunking loses context at boundaries. Offering optional overlap (repeat last N sentences of previous chunk in next chunk) improves retrieval quality for long sections. | Medium | Off by default (heading-only is clean and predictable). Enable with `--chunk-overlap=2` (number of sentences). Only applies within a single heading section that exceeds a max-chunk-size threshold. |
| Embedding model metadata tracking | When the user switches embedding models (e.g., Anthropic updates their model), all existing embeddings are invalid. The tool should detect this and trigger a full re-index. | Low | Store `model_id` in the database metadata. On startup, compare stored model_id against configured model. If different, warn and offer `--force-reindex`. |
| Prometheus metrics with meaningful histograms | Most local tools have zero observability. Exposing embed latency, search latency, queue depth, and error rates makes this tool production-grade. | Medium | Already planned. Differentiate by exposing *useful* metrics: p50/p95/p99 embed latency, search latency, chunks indexed per minute, API error rate, queue depth. |
| Web dashboard for debugging | A lightweight HTML UI that lets you search interactively, browse indexed files, and inspect individual chunks. Not a primary interface -- a debugging/exploration tool. | High | Serve static HTML + HTMX or similar. Keep it simple. The dashboard is for humans debugging the index, not a primary search UI. |
| Wiki-link resolution | Obsidian `[[note]]` links are meaningful. Resolving them to actual file paths and storing link relationships enables "find notes that reference X." | Medium | Parse `[[note]]` and `[[note|alias]]` syntax. Store as metadata on the chunk. Enable a `--related <file>` query mode that finds chunks linking to a given note. |

## Anti-Features

Features to deliberately NOT build.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Local embedding models | Massive complexity (ONNX runtime, model downloads, GPU detection, memory management). Violates the "single binary, zero deps" constraint. The Anthropic API is the right tradeoff for v1 -- the user already has an API key. | Use Anthropic API only. Design the embedding interface as a trait so a local model backend can be added later without refactoring. |
| PDF / DOCX / image ingestion | Each format needs a separate extraction pipeline, often with C dependencies (poppler, tesseract). Scope creep that delays shipping. Obsidian vaults are markdown. | Support `.md` files only in v1. Design the file processor as a trait so formats can be added later. Log a clear message when non-markdown files are skipped. |
| Multi-vault / multi-tenant support | Adds configuration complexity, database isolation concerns, and UI complications. One daemon per vault is simple and correct. | Single root directory per daemon instance. Users who want multiple vaults run multiple instances on different ports. Document this clearly. |
| LLM-powered query rewriting | Sending the user's query to an LLM to "improve" it before searching adds latency, cost, and unpredictability. Claude Code is already an LLM -- it can formulate good queries itself. | Return raw search results. Let the calling LLM (Claude Code) decide how to interpret and re-query if needed. |
| Authentication / user management | This is a local daemon on localhost. Auth adds complexity with zero security benefit (if someone has localhost access, auth doesn't help). | Bind to `127.0.0.1` only by default. Document that this is a local-only tool. Add a `--bind` flag for users who want to expose it (at their own risk). |
| Real-time collaborative editing awareness | Obsidian is single-user. Syncing vaults (Obsidian Sync, git) creates transient conflicts, but the watcher handles them as normal file changes. Building conflict resolution is pure waste. | Treat each file event as authoritative. If a sync conflict creates a `file (conflict).md`, index it as a separate file. |
| Semantic caching / query result caching | Adds complexity with minimal benefit. Vector search in LanceDB over a personal vault (typically <100K chunks) is fast enough (<50ms). Caching creates staleness bugs. | No query cache. Every search hits the index directly. If performance becomes an issue at scale, add caching then -- not before there's evidence of need. |
| Automatic summarization of chunks | Sending each chunk to an LLM for summarization during indexing multiplies API cost by ~10x and adds hours to initial index time. The chunk text itself is the summary. | Store raw chunk text. If summarization is needed, it's the consumer's job (Claude Code can summarize search results itself). |
| GUI configuration editor | Config via CLI flags and env vars is the right UX for a developer tool consumed primarily by another program (Claude Code). A settings UI is maintenance burden with near-zero usage. | Read-only config view in the web dashboard. All config changes via CLI flags, `.env` file, or environment variables. |
| Plugin / extension system | Premature abstraction. The tool has one job. An extension system adds API surface to maintain before you know what extensions people actually want. | Hardcode the pipeline: watch -> chunk -> embed -> store -> search. Design internal interfaces cleanly (traits) so the codebase is extensible by *contributors*, not by *plugins*. |

## Feature Dependencies

```
Heading-based chunking --> Incremental indexing (need chunk hashing to diff)
Incremental indexing --> File-system watching (daemon triggers incremental updates)
Semantic search --> Chunking + Embedding (need vectors in the DB)
Full-text search --> Chunking (need text in the DB)
Hybrid search --> Semantic search + Full-text search (fuses both)
Context window expansion --> Chunk ordering stored per file
Per-file/tag filtering --> Frontmatter awareness (tags parsed from frontmatter)
Wiki-link resolution --> Frontmatter awareness (both are metadata extraction)
Stale index detection --> Incremental indexing (uses same hash/mtime tracking)
Embedding model tracking --> Incremental indexing (invalidates hashes on model change)
Web dashboard --> HTTP server (serve subcommand)
Prometheus metrics --> HTTP server (same server, /metrics endpoint)
Skill files --> CLI working correctly (skills shell out to CLI)
```

## MVP Recommendation

Build in this order based on dependencies and value delivery:

**Phase 1 -- Core pipeline (must ship first):**
1. Heading-based markdown chunking with frontmatter parsing
2. Anthropic API embedding with retry/backoff
3. LanceDB storage with content hashing
4. Semantic search (vector ANN)
5. Structured JSON CLI output
6. One-shot index mode (`local-index index`)

**Phase 2 -- Live indexing and full search:**
7. Full-text search (Tantivy via LanceDB)
8. Hybrid search with RRF fusion
9. File-system watching (daemon mode)
10. Incremental updates (hash-based diff)
11. Configurable result count and score threshold

**Phase 3 -- Integration and observability:**
12. Claude Code skill files
13. Prometheus metrics endpoint
14. Stale index detection / status command
15. Embedding model metadata tracking

**Phase 4 -- Polish and differentiation:**
16. Context window expansion (prev/next chunks)
17. Per-file and per-tag filtering
18. Heading hierarchy ranking boost
19. Web dashboard
20. Wiki-link resolution

**Defer indefinitely:**
- Local embedding models: until there is a clear user demand and the trait interface is proven
- PDF/DOCX: until v2 when the core is stable
- Chunk overlap: until users report boundary-related retrieval failures

## How Competitors Handle Key Concerns

### Chunking Strategies

**Obsidian Smart Connections:** Chunks by heading (H1-H6). Each heading section becomes one embedding unit. Falls back to paragraph-level splitting for files without headings. Includes the heading hierarchy as prefix text in the chunk (e.g., "# Note Title > ## Section > ### Subsection: actual content"). This is the right approach for Obsidian vaults.

**Khoj:** Offers multiple chunkers (markdown heading, fixed-size with overlap, sentence-based). Markdown heading is the default for `.md` files. Fixed-size with overlap is used as a fallback when heading-based chunks exceed a max token limit.

**Recommendation for local-index:** Chunk by heading as primary strategy. If a heading section exceeds ~1500 tokens (roughly 6000 chars), split it into sub-chunks with 2-sentence overlap. Prefix each chunk with its heading breadcrumb. Store the breadcrumb as separate metadata for filtering.

### Re-indexing and Incremental Updates

**Obsidian Smart Connections:** Uses file mtime to detect changes. Re-embeds entire file when any part changes. This is wasteful -- if you edit one heading section, all sections get re-embedded.

**Khoj:** Content-hashes each chunk. On file change, re-chunks the file, compares chunk hashes against stored hashes, and only re-embeds changed chunks. Deletes chunks that no longer exist (heading removed or renamed). This is the correct approach.

**Recommendation for local-index:** Follow Khoj's pattern. Hash each chunk (SHA-256 of chunk text + heading breadcrumb). On file change: re-chunk the file, compare hashes, embed only new/changed chunks, delete orphaned chunks. This minimizes API cost and latency. Store file mtime as a fast first-pass filter (skip files whose mtime hasn't changed).

### Result Ranking

**Obsidian Smart Connections:** Pure cosine similarity ranking. No hybrid search. This means exact keyword matches can rank poorly if the semantic meaning doesn't align.

**Khoj:** Hybrid ranking via cross-encoder re-ranking. Initial retrieval via vector search, then re-ranks top-N with a cross-encoder model. Effective but requires a local model, which conflicts with the single-binary constraint.

**General RAG best practice:** Reciprocal Rank Fusion (RRF) is the standard for combining vector + full-text results without needing a separate re-ranking model. Formula: `RRF_score(d) = sum(1 / (k + rank_i(d)))` where k is typically 60 and rank_i is the rank in each result list.

**Recommendation for local-index:** Use RRF to fuse vector search results with Tantivy full-text results. This gives the benefits of hybrid search without requiring a local re-ranking model. LanceDB supports this pattern natively via its hybrid search API.

### What Claude Code Needs from a Search Tool

Based on how Claude Code consumes external tools via skill invocations:

1. **Structured JSON output** -- Claude Code parses JSON, not prose. Every field should be predictable.
2. **File paths relative to the vault root** -- Claude Code needs to construct `Read` tool calls from the paths. Absolute paths break portability.
3. **Chunk text with sufficient context** -- A 2-sentence snippet is too little; the full heading section is right. Optionally include surrounding chunks.
4. **Similarity/relevance score** -- Claude Code uses scores to decide whether to trust results or search again with different terms.
5. **Low-latency responses** -- Claude Code skill invocations block the conversation. Search must return in <500ms for good UX.
6. **Exit code conventions** -- 0 = success, 1 = no results found, 2 = error. Claude Code can branch on exit codes.
7. **Stderr for diagnostics, stdout for data** -- Claude Code captures stdout as the skill result. Logging/warnings must go to stderr only.
8. **Idempotent commands** -- Running `search` twice with the same query must return the same results (no side effects).
9. **Discoverability** -- Skill files in `.claude/skills/` with clear descriptions so Claude Code knows what tools are available and when to use them.

## Sources

- Training data knowledge of: Obsidian Smart Connections (v2.x), Khoj (self-hosted RAG), Haystack (document processing framework), LangChain (RAG patterns), LanceDB documentation, Anthropic embeddings API documentation
- Note: All findings are MEDIUM confidence. WebSearch and WebFetch were unavailable for live verification. Patterns described reflect the state of these tools as of early-to-mid 2025 from training data. Specific version numbers and API details should be verified against current documentation before implementation.
