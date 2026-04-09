# Domain Pitfalls

**Domain:** Rust file-indexing daemon with LanceDB and Anthropic embeddings
**Researched:** 2026-04-08
**Overall Confidence:** MEDIUM (web research tools unavailable; based on training data through mid-2025 -- verify LanceDB Rust API and Anthropic embeddings API details against current docs before implementation)

---

## Critical Pitfalls

Mistakes that cause rewrites, data loss, or production-blocking issues.

### Pitfall 1: LanceDB Concurrent Write Conflicts

**What goes wrong:** LanceDB uses a Lance columnar format with append-only fragment files and a manifest. Concurrent writers to the same table from the same or different processes can corrupt the manifest or silently drop writes. The Rust `lancedb` crate does not provide built-in write locking for embedded mode.

**Why it happens:** In daemon mode, a file-change burst (e.g., `git pull` updating 50 files) triggers many concurrent embedding+write operations. If you `tokio::spawn` each write independently, multiple tasks call `table.add()` concurrently. Lance's optimistic concurrency model retries on conflict, but under heavy contention retries can cascade and fail.

**Consequences:** Lost embeddings (chunks written but not visible in queries), corrupted table state requiring manual compaction or rebuild, or panics from the native Arrow layer.

**Prevention:**
- Funnel all LanceDB writes through a single writer task using an `mpsc` channel. Batch incoming chunks and write them in a single `table.add()` call per batch window (e.g., every 500ms or every 100 chunks, whichever comes first).
- Never allow parallel `table.add()` calls to the same table.
- Wrap the write path in an integration test that hammers concurrent inserts and verifies row counts.

**Detection:** Row counts after bulk indexing don't match expected chunk counts. Search misses recently-indexed files.

**Phase:** Phase 1 (core indexing). Design the single-writer pattern from day one.

---

### Pitfall 2: LanceDB Vector Index Creation Timing

**What goes wrong:** LanceDB only creates IVF-PQ (or similar ANN) vector indices explicitly via `create_index()`. Without an index, all vector searches are brute-force flat scans. Developers either forget to create the index, create it too early (on too few rows for meaningful centroids), or don't rebuild it after significant data changes.

**Why it happens:** Unlike PostgreSQL with pgvector where you CREATE INDEX once, LanceDB indices must be rebuilt when data distribution changes significantly. The index is a one-time snapshot of the data distribution at creation time. New rows appended after index creation are searched via flat scan and merged with ANN results, which works but degrades as the unindexed tail grows.

**Consequences:** Either all searches are slow (no index) or search quality degrades over time (stale index with large unindexed tail). For a vault with 10K+ chunks, brute-force search could take 100ms+ instead of <10ms.

**Prevention:**
- Create the vector index after initial bulk indexing completes (not during).
- Track the number of rows added since last index creation. Rebuild the index when unindexed rows exceed ~20% of total rows.
- Expose "index staleness" as a Prometheus metric.
- For small vaults (<1000 chunks), flat scan is fine -- skip index creation entirely and save complexity.

**Detection:** Search latency Prometheus histograms creeping up over time. The `status` CLI command should report index freshness.

**Phase:** Phase 2 (search). Implement alongside search, not during initial write path.

---

### Pitfall 3: Anthropic Embeddings API is Actually Voyage AI

**What goes wrong:** Anthropic does not (as of mid-2025) offer a first-party embeddings endpoint. They partner with Voyage AI and recommend `voyage-3` or `voyage-3-lite` models. The PROJECT.md references "Anthropic embeddings API" but the actual API endpoint, auth, rate limits, and pricing are Voyage AI's, not Anthropic's.

**Why it happens:** The Anthropic docs page on embeddings redirects to Voyage AI documentation. The API key, base URL, and request format are all different from the Claude API.

**Consequences:** Building against a non-existent Anthropic embeddings endpoint. Credential resolution assuming the same API key works for both Claude and embeddings. Architecture assuming Anthropic-style rate limits.

**Prevention:**
- **Verify current state first:** Check whether Anthropic has launched a native embeddings endpoint by the time you implement. This is the single highest-priority verification item.
- Design the embedding client behind a trait (`EmbeddingProvider`) so the backing service (Voyage, Anthropic native, or future alternatives) can be swapped.
- If Voyage AI: requires a separate API key (`VOYAGE_API_KEY`), separate rate limits (~300 RPM for voyage-3), separate token counting (Voyage uses its own tokenizer), and separate pricing (~$0.06/1M tokens for voyage-3-lite).
- If Anthropic has launched native embeddings: verify the model name, token limit per request, and whether the same API key works.

**Detection:** First API call during implementation will fail if targeting wrong endpoint.

**Phase:** Phase 1 (core indexing). This is a blocking architectural decision -- resolve before writing any embedding code.

**Confidence:** LOW -- this is the most likely area where training data is stale. Anthropic may have launched native embeddings since mid-2025.

---

### Pitfall 4: Blocking the Tokio Runtime with File I/O and LanceDB

**What goes wrong:** Reading markdown files from disk, parsing them, and LanceDB's internal Arrow/Parquet operations involve blocking I/O. Calling these from async Tokio tasks without `spawn_blocking` starves the Tokio runtime's cooperative scheduling, causing all async tasks (HTTP server, file watcher, metrics endpoint) to stall.

**Why it happens:** Rust's type system doesn't prevent calling blocking code from async context. `std::fs::read_to_string` compiles fine inside an `async fn`. LanceDB's Rust API may have async signatures but internally perform blocking Arrow compute. The `notify` crate's event receiver may also block.

**Consequences:** The daemon appears to hang: HTTP `/metrics` endpoint stops responding, file watcher events queue up, search requests time out. Under load (bulk re-index), the entire process becomes unresponsive.

**Prevention:**
- Use `tokio::task::spawn_blocking` for all file reads, markdown parsing, and any synchronous LanceDB operations.
- Better: dedicate a separate `std::thread` pool (via `rayon` or a manual thread pool) for CPU-bound markdown parsing and file I/O, communicating with the async world via channels.
- Run `tokio::runtime::Builder::new_multi_thread()` with explicit worker count, and monitor task scheduling latency.
- Use `tokio-console` during development to detect tasks that hold the runtime too long.

**Detection:** HTTP endpoints becoming intermittently unresponsive during bulk indexing. `tokio-console` showing blocked workers.

**Phase:** Phase 1 (core architecture). The async/sync boundary must be designed correctly from the start.

---

### Pitfall 5: File Watcher Event Storms and Missing Debounce

**What goes wrong:** The `notify` crate fires events for every individual file operation. Saving a file in most editors involves: write to temp file, rename temp to target (atomic save), which generates Create + Modify + Rename events. Git operations generate hundreds of events in milliseconds. Without debouncing, each event triggers a re-index pipeline run.

**Why it happens:** Developers test with manual single-file saves and miss the bulk-operation case. The `notify` crate v6+ removed built-in debouncing, requiring manual implementation or the `notify-debouncer-full` crate.

**Consequences:** Redundant embedding API calls (expensive), database write storms, and potential API rate limit exhaustion from a single `git pull`.

**Prevention:**
- Use `notify-debouncer-full` or implement a manual debounce window (300-500ms) that coalesces events per file path.
- After the debounce window, deduplicate by file path -- only the latest event per path matters.
- Add a "pending re-index queue" with bounded capacity. If the queue is full, merge/replace pending items rather than dropping.
- Track "events received" vs "re-index operations triggered" as Prometheus counters to verify debouncing effectiveness.

**Detection:** Prometheus metrics showing embedding API calls >> file change events. Log lines showing the same file re-indexed multiple times in quick succession.

**Phase:** Phase 1 (file watching). Implement debouncing from the first file watcher iteration.

---

### Pitfall 6: macOS FSEvents vs Linux inotify Platform Differences

**What goes wrong:** macOS FSEvents delivers events at the directory level, not file level, and can deliver them in batches with significant delay (up to several seconds). Events may arrive out of order. Rename events on macOS don't always pair correctly. Linux inotify is file-level and more immediate but has a per-user watch limit (`/proc/sys/fs/inotify/max_user_watches`, default 8192 on many distros).

**Why it happens:** The `notify` crate abstracts over platform backends but the abstraction leaks. Code tested on macOS works differently on Linux and vice versa.

**Consequences:**
- macOS: Delayed or duplicate indexing. Rename detection fails, causing a delete+create instead of a move (losing change history).
- Linux: `notify` silently stops watching directories when the inotify watch limit is exceeded. Large vaults (1000+ directories) hit this limit.

**Prevention:**
- On macOS: treat all events as "file may have changed, re-check" rather than trusting event types. Use content hashing (file mtime + size, or xxhash of content) to detect actual changes.
- On Linux: document the inotify watch limit in deployment docs. Check the limit at startup and warn if it's too low. Use `sysctl fs.inotify.max_user_watches=65536` in docs.
- Test on both platforms in CI. At minimum, have integration tests that exercise the watcher.
- Consider a hybrid approach: file watcher for real-time + periodic full-scan (every 5 min) to catch anything the watcher missed.

**Detection:** Files that should be indexed aren't showing up in search. Startup log warning about inotify limits.

**Phase:** Phase 1 (file watching) for basic support. Phase 2 for periodic full-scan fallback.

---

## Moderate Pitfalls

### Pitfall 7: Markdown Chunking -- YAML Frontmatter as Content

**What goes wrong:** Obsidian files start with YAML frontmatter (`---\ntags: [foo]\n---`). If the chunker doesn't strip frontmatter before splitting by headings, the frontmatter becomes part of the first chunk and pollutes the embedding with metadata noise (tags, aliases, dates) rather than semantic content.

**Prevention:**
- Strip YAML frontmatter before chunking. Parse it separately and store as structured metadata on the chunk record (tags, aliases, dates become filterable fields).
- Use a proper YAML parser (`serde_yaml`) for frontmatter, not regex. Frontmatter can contain multi-line strings, nested objects, and special characters.
- Handle malformed frontmatter gracefully: if YAML parsing fails, treat the whole block as content rather than crashing.

**Detection:** Search results returning frontmatter metadata as matching content. Tags and dates appearing in semantic search snippets.

**Phase:** Phase 1 (markdown parsing).

---

### Pitfall 8: Markdown Chunking -- Wiki-Links and Obsidian Syntax

**What goes wrong:** Obsidian uses `[[note-name]]`, `[[note-name|display text]]`, `![[embedded-note]]`, and `![[image.png]]` syntax. These are not standard Markdown. If passed raw to the embedding API, the brackets and pipe syntax add noise. If aggressively stripped, you lose the semantic connection ("see [[Related Concept]]" becomes "see" which is meaningless).

**Prevention:**
- Convert wiki-links to their display text: `[[note-name]]` becomes `note-name`, `[[note-name|display text]]` becomes `display text`.
- Strip embed syntax `![[...]]` entirely (embedded images/notes aren't useful text).
- Store the raw wiki-link targets as metadata on the chunk (enables future cross-reference features).
- Handle edge cases: `[[note with (parens)]]`, `[[note#heading]]` (heading links), `[[note#^block-id]]` (block references).

**Detection:** Search results containing literal `[[` brackets. Embedding token counts higher than expected due to syntax noise.

**Phase:** Phase 1 (markdown parsing).

---

### Pitfall 9: Empty and Degenerate Chunks

**What goes wrong:** Splitting by heading produces empty chunks: a heading with no content before the next heading, a heading followed only by wiki-link embeds (which get stripped), or a heading that is the entire section. These empty/near-empty chunks waste embedding API calls and pollute search results.

**Prevention:**
- After chunking and syntax cleanup, filter out chunks below a minimum content threshold (e.g., <20 characters of actual text after stripping).
- Merge orphan chunks upward: if a heading has no content, merge it with the previous chunk or skip it.
- Track "chunks skipped (too short)" as a metric to monitor content extraction quality.

**Detection:** Prometheus metric showing high skip rate. Search results returning chunks with only a heading and no useful content.

**Phase:** Phase 1 (markdown parsing).

---

### Pitfall 10: LanceDB Schema Migration on Embedding Model Change

**What goes wrong:** If you change embedding models (different dimension, different model version), all existing vectors become incompatible. LanceDB has no built-in schema migration. You can't mix 1024-dim and 1536-dim vectors in the same column.

**Prevention:**
- Store the embedding model name and dimension in a metadata table or as a column on the chunks table.
- At startup, compare the configured model against what's stored. If they differ, require an explicit `--reindex` flag rather than silently producing mixed-dimension data.
- Design the schema with a `model_version` column from day one.
- Implement `--reindex` as "drop table, recreate, re-embed everything" -- simpler and safer than migration.

**Detection:** Startup check compares configured model vs stored model. Panic-early if mismatched and no `--reindex` flag.

**Phase:** Phase 1 (schema design). The model_version column must be in the initial schema.

---

### Pitfall 11: Embedding API Token Limits Per Request

**What goes wrong:** Embedding APIs have per-request token limits (Voyage: 32K tokens per batch, with individual text inputs limited to ~16K tokens depending on model). Long markdown sections can exceed the per-text limit. A batch of many chunks can exceed the per-request limit.

**Prevention:**
- Implement chunk size limiting: if a single chunk exceeds ~8000 tokens (conservative estimate), split it further at paragraph boundaries. Heading-based chunking alone isn't sufficient for files with very long sections.
- Count tokens before batching. Use a conservative estimate (4 chars per token for English) or integrate a proper tokenizer.
- Batch chunks respecting both per-text and per-batch token limits.
- Log and metric when chunks require secondary splitting (indicates heading structure is too coarse).

**Detection:** 400 errors from the embedding API mentioning token limits. Chunks that fail to embed silently.

**Phase:** Phase 1 (embedding pipeline).

---

### Pitfall 12: Embedding API Rate Limiting and Cost Surprises

**What goes wrong:** Initial bulk indexing of a large vault (10K+ files, 50K+ chunks) can exhaust rate limits quickly and run up unexpected costs. Voyage voyage-3 is ~300 RPM, voyage-3-lite is ~300 RPM with higher throughput per request. A 50K chunk re-index at 128 chunks/batch = ~400 requests, at 300 RPM = ~1.3 minutes minimum. Cost at ~$0.06/1M tokens for voyage-3-lite with average 200 tokens/chunk = 10M tokens = ~$0.60. But with voyage-3 at $0.13/1M tokens = ~$1.30.

**Prevention:**
- Implement a rate limiter (use `governor` crate or manual token bucket) in the embedding client, not just retry-on-429.
- Log estimated cost before starting bulk re-index. Warn if above a threshold (e.g., >$1).
- Implement `--dry-run` for bulk indexing that reports chunk count and estimated cost without calling the API.
- Use exponential backoff on 429 responses with jitter.
- Default to the cheaper/faster lite model for v1.

**Detection:** 429 responses in logs. Cost tracking via token count metrics.

**Phase:** Phase 1 (embedding pipeline). Rate limiter must be in place before bulk indexing is possible.

---

### Pitfall 13: Backpressure in the Indexing Pipeline

**What goes wrong:** Without backpressure, the pipeline stages (file watcher -> file reader -> chunker -> embedder -> writer) can overwhelm downstream stages. The file watcher produces events faster than the embedding API can consume them, causing unbounded memory growth in intermediate queues.

**Prevention:**
- Use bounded channels (`tokio::sync::mpsc::channel(N)`) between every pipeline stage.
- When the embedding queue is full, the chunker blocks, which blocks the file reader, which blocks the file watcher event processing. This is correct behavior.
- Size bounds based on the bottleneck: the embedding API. If the API can handle 300 RPM with 128 chunks/batch, the pipeline can process ~600 chunks/second max. Size queues at 2-3x this rate.
- Expose queue depths as Prometheus gauges.

**Detection:** Memory usage growing linearly during bulk indexing (OOM). Queue depth metrics showing unbounded growth.

**Phase:** Phase 1 (pipeline architecture).

---

### Pitfall 14: Prometheus Histogram Bucket Misconfiguration

**What goes wrong:** Default histogram buckets in `prometheus` crate (0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0 seconds) are wrong for this workload. Anthropic/Voyage API calls take 200ms-2s typically. File reads take 1-50ms. Search queries take 5-100ms. Default buckets miss the interesting distributions.

**Prevention:**
- Define custom bucket sets per metric type:
  - API calls: `[0.1, 0.2, 0.3, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0]`
  - File I/O: `[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5]`
  - Search: `[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]`
- Do NOT add high-cardinality labels (e.g., file path, query text) to histograms. Use only bounded label sets: operation type, status code, model name.

**Detection:** Histogram quantiles all reading 0 or +Inf (all observations falling outside bucket range). Grafana dashboards showing no useful latency distribution.

**Phase:** Phase 3 (observability). But define the bucket constants during Phase 1 when implementing the metrics.

---

### Pitfall 15: Cardinality Explosion in Prometheus Metrics

**What goes wrong:** Adding labels like `file_path`, `chunk_id`, or `query` to Prometheus metrics creates a unique time series per unique value. An Obsidian vault with 10K files and per-file metrics = 10K time series per metric. Prometheus scraping becomes slow, memory usage explodes.

**Prevention:**
- Label metrics only with bounded enums: `operation={index,search,delete}`, `status={success,error}`, `model={voyage-3-lite}`.
- Never use file paths, chunk IDs, query text, or user input as label values.
- Use counters for per-file tracking if needed (e.g., `files_indexed_total` counter, not a gauge per file).
- Test with a large vault and check `/metrics` endpoint response size and scrape duration.

**Detection:** `/metrics` endpoint response > 1MB. Scrape duration > 1s.

**Phase:** Phase 3 (observability). But set the policy during Phase 1.

---

### Pitfall 16: LanceDB Compaction Neglect

**What goes wrong:** LanceDB stores data in append-only fragments. Every `table.add()` call creates a new fragment. With incremental indexing (one file changed = one small write), fragments proliferate. Read performance degrades as LanceDB must open and read many small files.

**Prevention:**
- Implement periodic compaction: after N writes or on a timer (e.g., every 10 minutes of daemon uptime), call `table.optimize()` or the compaction API.
- Batch writes (see Pitfall 1) to reduce fragment creation rate.
- Track fragment count as a metric (if LanceDB exposes it) or proxy via write count since last compaction.
- Compact during idle periods (no pending writes for 30 seconds).

**Detection:** Search latency degrading over time. Storage directory showing thousands of small parquet files.

**Phase:** Phase 2 (performance). Initial implementation can defer compaction, but it must be added before the daemon runs for extended periods.

---

### Pitfall 17: Credential File Format Instability

**What goes wrong:** The `~/.claude/` credential store format is not a documented stable API. Claude Code updates can change the file structure, field names, or add new required fields. Parsing it with hardcoded field expectations breaks silently on update.

**Prevention:**
- Prefer `ANTHROPIC_API_KEY` environment variable as the primary credential source. Document this clearly.
- For `~/.claude/` fallback: implement defensive parsing with `serde_json` using `#[serde(default)]` and optional fields everywhere.
- Log a clear warning if `~/.claude/` parsing fails, with the specific parse error, rather than a generic "no credentials" message.
- Test with an empty file, missing fields, extra fields, and wrong types. Don't panic on any of these.
- Pin to known field names and document which Claude Code version the format was verified against.

**Detection:** Daemon fails to start after a Claude Code update. Error message mentioning credential parsing.

**Phase:** Phase 1 (credentials). But design it as a best-effort fallback, not the primary path.

---

## Minor Pitfalls

### Pitfall 18: Symlink Loops in Obsidian Vaults

**What goes wrong:** Users sometimes symlink directories into their vault (e.g., linking a shared notes folder). Symlink loops cause the file watcher and directory walker to recurse infinitely.

**Prevention:**
- Use `walkdir` crate with `follow_links(false)` for directory traversal, or `follow_links(true)` with `max_depth` and explicit loop detection.
- Track visited inodes/device pairs to detect loops.
- The `notify` crate with `RecursiveMode::Recursive` follows symlinks on some platforms -- test this explicitly.

**Detection:** 100% CPU during directory scan. Stack overflow or "too many open files" errors.

**Phase:** Phase 1 (file walking).

---

### Pitfall 19: Very Long Headings as Chunk Identifiers

**What goes wrong:** Using the full heading text as a chunk identifier or database key creates unwieldy keys. Some Obsidian users write headings that are full sentences (100+ characters). If used as a composite key with file path, you may exceed reasonable key lengths.

**Prevention:**
- Use a deterministic hash (xxhash or blake3 of `file_path + heading_path + heading_text`) as the chunk ID.
- Store the heading text as a separate metadata column for display, not as the primary key.

**Detection:** Database queries becoming slow due to long key comparisons. Display truncation issues in web UI.

**Phase:** Phase 1 (schema design).

---

### Pitfall 20: Single Binary with LanceDB Native Dependencies

**What goes wrong:** LanceDB's Rust crate (`lancedb`) depends on Arrow, Parquet, and potentially native libraries (OpenSSL, system allocators). Cross-compilation and static linking can be problematic. The resulting binary may be large (50MB+) and may not be truly portable between Linux distributions if dynamically linked.

**Prevention:**
- Use `cargo build --release` with `lto = true` and `strip = true` in the release profile to minimize binary size.
- Test the binary on a clean system (Docker container with minimal base image) to verify no missing shared libraries.
- On Linux, consider `musl` target (`x86_64-unknown-linux-musl`) for a fully static binary. Verify LanceDB/Arrow compile with musl.
- On macOS, the binary will link against system frameworks -- this is fine for local use but means macOS builds aren't portable to Linux and vice versa.

**Detection:** Binary fails to run on a different machine with "shared library not found" errors. Build failure on musl target.

**Phase:** Phase 4 (packaging/deployment). Don't worry about this until the daemon works correctly.

---

### Pitfall 21: `tracing` Subscriber Configuration Conflicts

**What goes wrong:** Multiple crates in the dependency tree may try to set a global `tracing` subscriber. Setting it twice panics. The `prometheus` metrics crate and the `tracing` crate may fight over global state.

**Prevention:**
- Set the global subscriber exactly once, in `main()`, before any other initialization.
- Use `tracing_subscriber::registry()` with layers (formatting layer + optional JSON layer + optional OpenTelemetry layer) rather than the convenience `fmt::init()`.
- If using `metrics` crate alongside `tracing`, they are separate concerns and don't conflict. But `tracing-opentelemetry` + `prometheus` exporter can have initialization order issues.

**Detection:** Panic at startup: "a]global default trace dispatcher has already been set".

**Phase:** Phase 1 (bootstrap/main function).

---

### Pitfall 22: Incremental Indexing -- Detecting Actual Changes

**What goes wrong:** File watcher says "file modified" but the content relevant to indexing hasn't changed (e.g., only file metadata/mtime changed, or a save without edits). Re-embedding unchanged content wastes API calls.

**Prevention:**
- Store a content hash (xxhash of the raw file content, or of the extracted chunks) alongside each file's record.
- On "file modified" event: read file, hash content, compare to stored hash. Only re-chunk and re-embed if the hash changed.
- This also handles the case where the editor saves without changes (e.g., `:w` in vim on an unmodified buffer).

**Detection:** Metrics showing re-index operations where zero chunks actually changed. Embedding API costs higher than expected for the actual edit rate.

**Phase:** Phase 1 (incremental indexing).

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Schema design | Pitfall 10: No model_version column | Add model_version and embedding_dim to schema from day one |
| File watching | Pitfall 5, 6: Event storms + platform differences | Debounce + content hashing, test on both macOS and Linux |
| Embedding pipeline | Pitfall 3: Wrong API endpoint | Verify whether Anthropic has native embeddings before writing code |
| Embedding pipeline | Pitfall 11, 12: Token limits + rate limits | Rate limiter + token counting before any batch API call |
| Pipeline architecture | Pitfall 4, 13: Blocking + backpressure | spawn_blocking for I/O, bounded channels between stages |
| Write path | Pitfall 1: Concurrent writes | Single-writer task with batched writes from day one |
| Search | Pitfall 2: Missing/stale vector index | Index creation after bulk load, staleness tracking |
| Observability | Pitfall 14, 15: Histogram buckets + cardinality | Custom buckets per metric type, bounded label policy |
| Credentials | Pitfall 17: Format instability | Env var as primary, defensive parsing as fallback |
| Deployment | Pitfall 20: Native deps | Test on clean system, consider musl for Linux |
| Markdown parsing | Pitfall 7, 8, 9: Frontmatter + wiki-links + empty chunks | Strip frontmatter, convert wiki-links, filter degenerate chunks |

## Sources

- Training data knowledge of LanceDB, Voyage AI, notify crate, Tokio runtime, Prometheus Rust ecosystem (through mid-2025)
- Unable to verify against current documentation (WebSearch and WebFetch tools unavailable)
- **Key verification needed:** Pitfall 3 (Anthropic embeddings API status) is the highest-priority item to verify against current docs before implementation
