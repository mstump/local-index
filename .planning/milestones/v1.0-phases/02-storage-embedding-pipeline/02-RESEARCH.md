# Phase 2: Storage & Embedding Pipeline - Research

**Researched:** 2026-04-09
**Domain:** Voyage AI embeddings, LanceDB embedded vector storage, incremental indexing
**Confidence:** MEDIUM (lancedb evolves rapidly; arrow version pinning is fragile)

## Summary

Phase 2 replaces the JSONL output from Phase 1's index command with a full embedding and storage pipeline. Chunks produced by the walker/chunker are hashed (SHA-256), embedded via the Voyage AI HTTP API, and stored in an embedded LanceDB database. Incremental re-indexing skips unchanged chunks by comparing content hashes. The CLI gains TTY-aware progress reporting via indicatif.

The Voyage AI embedding API is a straightforward REST endpoint (`POST /v1/embeddings`) that accepts batches of text and returns float vectors. The default model `voyage-3.5` produces 1024-dimensional embeddings. LanceDB's Rust crate (v0.26.2, the latest compatible with our rustc 1.89.0) provides async table creation, RecordBatch insertion, and vector search via Arrow types. FTS is confirmed available in the Rust crate via `Index::FTS`.

**Primary recommendation:** Use lancedb 0.26.2 with arrow 57.x. Build a thin `VoyageEmbedder` over reqwest. Use `merge_insert` for upsert semantics on content hash. Use indicatif 0.18.4 with automatic TTY detection (draws to stderr; hidden when non-TTY).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Voyage AI is the sole embedding provider in Phase 2. Google Gemini is deferred.
- **D-02:** Embedding is behind an `Embedder` trait so future providers (Gemini, OpenAI, local models) can be added without changing the pipeline.
- **D-03:** The concrete implementation is `VoyageEmbedder`.
- **D-04:** Credential resolution for Voyage AI checks `VOYAGE_API_KEY` env var only. There is no `~/.claude/` fallback.
- **D-05:** Startup fails with a clear, actionable error message if `VOYAGE_API_KEY` is not set (CRED-03).
- **D-06:** Output mode is auto-detected via TTY check on stdout (`std::io::IsTerminal`).
- **D-07:** Interactive mode (TTY): indicatif progress bar showing files processed / chunks embedded / errors, followed by a human-readable summary line.
- **D-08:** Agent/pipe mode (non-TTY): progress lines to stderr, one JSON summary object to stdout on completion.
- **D-09:** SHA-256 hash is computed over: body + heading_breadcrumb + serialized frontmatter.

### Claude's Discretion
- Voyage AI model name and embedding dimensions (researcher to confirm from Voyage AI docs)
- LanceDB table schema column layout (follow INDX-05 requirements)
- Exact exponential backoff parameters (base delay, jitter range, max retries) -- follow INDX-07
- indicatif bar style and tick rate

### Deferred Ideas (OUT OF SCOPE)
- Google Gemini `Embedder` implementation -- deferred from Phase 2
- `~/.claude/` credential fallback -- not applicable to Voyage AI
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | `local-index index <path>` performs one-shot full index and exits | Main.rs already has the command wired; Phase 2 replaces JSONL output with embed+store pipeline |
| CRED-01 | Credential resolution checks env var | D-04 locks this to `VOYAGE_API_KEY` env var only (no ~/.claude/ fallback) |
| CRED-02 | Embedder trait with Voyage AI implementation | D-02/D-03 lock the trait + VoyageEmbedder approach; Voyage API shape documented below |
| CRED-03 | Startup fails with clear error if no credentials | D-05 locks this; check env var at startup, fail with actionable message |
| INDX-04 | SHA-256 content hash for incremental re-indexing | D-09 defines hash input; sha2 crate v0.11.0 for computation; store hash in LanceDB, compare on re-index |
| INDX-05 | LanceDB schema with all chunk metadata | Arrow schema with FixedSizeList for embedding vector; see schema design below |
| INDX-06 | Model mismatch guard requiring --force-reindex | Store model ID in a metadata table or as a column; compare at startup |
| INDX-07 | Exponential backoff with jitter on transient errors | reqwest + manual retry loop with tokio::time::sleep; see backoff pattern below |
| INDX-08 | Progress reporting during one-shot index | indicatif 0.18.4 with TTY auto-detection; D-06/D-07/D-08 define output modes |
</phase_requirements>

## Standard Stack

### Core (New Dependencies for Phase 2)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| lancedb | 0.26.2 | Embedded vector database | Project requirement. 0.27.2 requires rustc 1.91+; our nightly is 1.89.0. 0.26.2 is the latest compatible version. |
| arrow-array | ^57.2 | Arrow array types for RecordBatch | Must match lancedb 0.26.2's arrow dependency (^57.2). Latest 58.x is incompatible. |
| arrow-schema | ^57.2 | Arrow schema/field/datatype definitions | Same version constraint as arrow-array for lancedb 0.26.2 compatibility. |
| reqwest | ^0.12 | HTTP client for Voyage AI API | Already recommended in CLAUDE.md. Thin wrapper approach for single endpoint. Note: 0.13.2 exists but 0.12 is proven stable. |
| sha2 | ^0.11 | SHA-256 content hashing | Standard Rust crypto crate for digest computation. |
| indicatif | ^0.18 | Progress bar rendering | The standard Rust CLI progress library. Auto-hides when non-TTY. Draws to stderr by default. |
| futures | ^0.3 | Stream/TryStreamExt for RecordBatch collection | Required for `try_collect()` on LanceDB query results. |

### Already Present (from Phase 1)

| Library | Version | Purpose |
|---------|---------|---------|
| tokio | ^1.40 (full) | Async runtime -- needed for lancedb async API |
| serde | ^1.0 | Serialization for API types |
| serde_json | ^1.0 | JSON for Voyage API request/response |
| anyhow | ^1.0 | Application-layer error handling |
| thiserror | ^2.0 | Library error enums |
| tracing | ^0.1 | Structured logging |
| clap | ^4.5 | CLI (--force-reindex flag already wired) |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| lancedb 0.27.2 | lancedb 0.26.2 | 0.27.2 needs rustc 1.91+; we have 1.89.0-nightly. Use 0.26.2. |
| arrow 58.x | arrow 57.x | Must match lancedb 0.26.2's dep constraint (^57.2). |
| reqwest 0.13 | reqwest 0.12 | 0.13 is available but 0.12 is what CLAUDE.md recommends and lancedb uses internally. |
| backoff crate | Manual retry loop | For a single retry site, a manual loop with tokio::time::sleep is simpler than adding a crate. |
| indicatif 0.18 | console + manual rendering | indicatif handles TTY detection, bar formatting, thread safety. No reason to hand-roll. |

**Installation:**
```bash
cargo add lancedb@0.26.2
cargo add arrow-array@57 arrow-schema@57
cargo add reqwest@0.12 --features json,rustls-tls
cargo add sha2@0.11
cargo add indicatif@0.18
cargo add futures@0.3
cargo add wiremock@0.6 --dev
```

## Architecture Patterns

### Recommended Project Structure
```
src/
  pipeline/
    mod.rs            # pub mod walker; pub mod chunker; pub mod embedder; pub mod store;
    walker.rs         # (existing)
    chunker.rs        # (existing)
    embedder.rs       # Embedder trait + VoyageEmbedder
    store.rs          # LanceDB store: open/create table, upsert chunks, query hashes
  types.rs            # (existing) Chunk, Frontmatter + new IndexedChunk, EmbeddingResult
  error.rs            # (existing) + new variants: Embedding, Database, Credential
  credentials.rs      # Credential resolution (VOYAGE_API_KEY env var)
  cli.rs              # (existing)
  lib.rs              # pub mod credentials; (add)
  main.rs             # Updated index command with embed+store+progress pipeline
```

### Pattern 1: Embedder Trait

**What:** Trait abstraction over embedding providers
**When to use:** Always -- all embedding goes through this trait
**Example:**
```rust
// Source: CONTEXT.md D-02, Voyage AI API docs
use async_trait::async_trait; // or use native async fn in trait (Rust 1.75+)

pub struct EmbeddingResult {
    pub embeddings: Vec<Vec<f32>>,
    pub model: String,
    pub total_tokens: u64,
}

// With Rust edition 2024 + nightly, async fn in traits works natively
pub trait Embedder: Send + Sync {
    async fn embed(&self, texts: &[String]) -> Result<EmbeddingResult, LocalIndexError>;
    fn model_id(&self) -> &str;
    fn dimensions(&self) -> usize;
}

pub struct VoyageEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dimensions: usize,
}
```

### Pattern 2: LanceDB Schema Design (INDX-05)

**What:** Arrow schema for the chunks table
**When to use:** Table creation and data insertion
**Example:**
```rust
// Source: LanceDB examples, INDX-05 requirements
use std::sync::Arc;
use arrow_schema::{DataType, Field, Schema};

const EMBEDDING_DIM: i32 = 1024; // Voyage AI default

fn chunks_schema() -> Schema {
    Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),          // UUID
        Field::new("file_path", DataType::Utf8, false),          // vault-relative
        Field::new("heading_breadcrumb", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),               // chunk text
        Field::new("line_start", DataType::UInt32, false),
        Field::new("line_end", DataType::UInt32, false),
        Field::new("frontmatter_json", DataType::Utf8, true),    // serialized JSON
        Field::new("content_hash", DataType::Utf8, false),       // SHA-256 hex
        Field::new("embedding_model", DataType::Utf8, false),    // model ID
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIM,
            ),
            false,
        ),
    ])
}
```

### Pattern 3: Content Hash for Incremental Re-indexing (INDX-04)

**What:** SHA-256 hash of chunk content for skip-if-unchanged
**When to use:** Before embedding each chunk, compare hash with stored hash
**Example:**
```rust
// Source: D-09 from CONTEXT.md
use sha2::{Sha256, Digest};

fn compute_content_hash(chunk: &Chunk) -> String {
    let mut hasher = Sha256::new();
    hasher.update(chunk.body.as_bytes());
    hasher.update(chunk.heading_breadcrumb.as_bytes());
    // Serialize frontmatter deterministically
    let fm_json = serde_json::to_string(&chunk.frontmatter)
        .unwrap_or_default();
    hasher.update(fm_json.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

### Pattern 4: Upsert via merge_insert or delete+insert

**What:** Insert new chunks, update changed chunks, skip unchanged
**When to use:** During re-indexing
**Example approach:**
```rust
// For incremental indexing:
// 1. Query existing hashes for the file being processed
// 2. Compare with computed hashes
// 3. Only embed chunks with new/changed hashes
// 4. Delete chunks for removed headings (file_path match, chunk_id not in new set)
// 5. Insert new/updated chunks

// Delete old chunks for a file:
table.delete(&format!("file_path = '{}'", file_path)).await?;
// Insert new chunks:
table.add(record_batch).execute().await?;
```

**Note on merge_insert:** While LanceDB supports `merge_insert` for upsert, the simpler pattern for this use case is delete-all-for-file + insert-new. This avoids the overhead of scanning the merge key column and is correct because we process entire files at a time.

### Pattern 5: Exponential Backoff with Jitter (INDX-07)

**What:** Retry transient API errors with increasing delay
**When to use:** Voyage AI API calls
**Example:**
```rust
use std::time::Duration;
use tokio::time::sleep;

const MAX_RETRIES: u32 = 5;
const BASE_DELAY_MS: u64 = 500;
const MAX_DELAY_MS: u64 = 30_000;

async fn embed_with_retry(
    embedder: &dyn Embedder,
    texts: &[String],
) -> Result<EmbeddingResult, LocalIndexError> {
    let mut attempt = 0;
    loop {
        match embedder.embed(texts).await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_transient() && attempt < MAX_RETRIES => {
                attempt += 1;
                let base = BASE_DELAY_MS * 2u64.pow(attempt - 1);
                let jitter = rand::random::<u64>() % (base / 2);
                let delay = Duration::from_millis((base + jitter).min(MAX_DELAY_MS));
                tracing::warn!(
                    attempt, max = MAX_RETRIES, delay_ms = delay.as_millis(),
                    "transient API error, retrying: {}", e
                );
                sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Pattern 6: Model Mismatch Guard (INDX-06)

**What:** Detect when configured embedding model differs from stored model
**When to use:** At index command startup, before processing any chunks
**Example approach:**
```rust
// Store model metadata in a separate LanceDB table or as first-row convention
// On startup:
// 1. Open chunks table
// 2. Query for distinct embedding_model values
// 3. If any stored model != configured model AND --force-reindex not set:
//    - Print warning with stored vs configured model names
//    - Exit with error code + message suggesting --force-reindex
// 4. If --force-reindex: drop all data and re-embed everything
```

### Anti-Patterns to Avoid
- **Embedding one chunk at a time:** Voyage AI supports batch requests (up to 1000 items, up to 120K-1M tokens depending on model). Always batch chunks for efficiency.
- **Storing frontmatter as separate Arrow columns:** Frontmatter has variable fields (the `extra` HashMap). Serialize as JSON string in a single column. Structured fields (tags, aliases) can be extracted to separate columns if search filtering is needed in Phase 3.
- **Using lancedb 0.27.x:** Will fail to compile with rustc 1.89.0-nightly. Pin to 0.26.2.
- **Using arrow 58.x:** Incompatible with lancedb 0.26.2's ^57.2 constraint. Pin to 57.x.
- **Blocking the async runtime with hash computation:** SHA-256 on small text chunks is fast enough to run inline. Don't spawn_blocking for this.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Progress bars | Custom terminal escape codes | indicatif 0.18 | TTY detection, thread safety, multi-bar support, clean API |
| SHA-256 hashing | Manual bit manipulation | sha2 0.11 | Audited crypto implementation, trait-based API |
| HTTP client | Raw TCP/TLS | reqwest 0.12 | Connection pooling, JSON support, TLS, timeouts |
| Arrow RecordBatch | Manual memory layout | arrow-array 57.x | LanceDB requires Arrow format; arrow crate is the only option |
| Vector database | Custom file format | lancedb 0.26.2 | Project requirement; handles indexing, compression, search |

## Common Pitfalls

### Pitfall 1: Arrow Version Mismatch
**What goes wrong:** Compilation fails with opaque type mismatch errors between arrow types
**Why it happens:** lancedb 0.26.2 depends on arrow ^57.2. If you add arrow 58.x, cargo will pull both versions and types won't be compatible.
**How to avoid:** Pin arrow-array and arrow-schema to ^57.2 explicitly in Cargo.toml
**Warning signs:** "expected arrow_array::RecordBatch, found arrow_array::RecordBatch" errors (two different versions)

### Pitfall 2: lancedb Requires rustc 1.91+ at Latest
**What goes wrong:** `cargo build` fails on lancedb 0.27.x with MSRV error
**Why it happens:** Project uses rustc 1.89.0-nightly. lancedb 0.27.2 requires 1.91+.
**How to avoid:** Pin lancedb to 0.26.2 in Cargo.toml
**Warning signs:** Cargo resolution warnings about ignoring versions

### Pitfall 3: Forgetting to Batch Embedding Requests
**What goes wrong:** Extremely slow indexing -- one API call per chunk instead of per batch
**Why it happens:** Natural to embed one chunk at a time in a loop
**How to avoid:** Collect chunks into batches (e.g., 50-100 chunks per request, staying under token limits). Voyage AI allows up to 1000 items per request.
**Warning signs:** Indexing takes minutes for small vaults

### Pitfall 4: Non-Deterministic Frontmatter Serialization
**What goes wrong:** Content hash changes on re-index even though nothing changed, causing unnecessary re-embedding
**Why it happens:** HashMap serialization order is non-deterministic in serde_json by default
**How to avoid:** Use `serde_json::to_string` which produces deterministic output for serde_json (keys are sorted for HashMap with string keys in serde_json). Alternatively, use BTreeMap in the `extra` field, or sort keys explicitly before hashing.
**Warning signs:** "Changed" chunks on every re-index with identical content

### Pitfall 5: Not Handling Partial Failures
**What goes wrong:** A single API error loses all progress from the current run
**Why it happens:** Embedding batch fails, and no chunks from that batch get stored
**How to avoid:** Store successfully embedded chunks immediately after each batch completes. Failed batches get retried independently. Use per-file or per-batch granularity for storage commits.
**Warning signs:** Re-running index after a failure re-embeds everything

### Pitfall 6: Blocking Async Runtime with LanceDB Operations
**What goes wrong:** Performance degradation or deadlocks
**Why it happens:** LanceDB operations are async but some internal operations may be CPU-intensive
**How to avoid:** All LanceDB operations use `.execute().await` -- this is the correct async pattern. Don't wrap in `spawn_blocking`.
**Warning signs:** Unexplained hangs during large table operations

### Pitfall 7: serde_json HashMap Key Ordering
**What goes wrong:** Content hashes differ between runs for identical content
**Why it happens:** The `Frontmatter.extra` field is `HashMap<String, serde_yml::Value>`. Standard HashMap iteration order is non-deterministic.
**How to avoid:** When computing the hash, either (a) use `serde_json::to_string` on a `BTreeMap` copy of extra, or (b) sort the serialized keys before hashing. The simplest fix: change `extra` to `BTreeMap<String, serde_yml::Value>` in `Frontmatter`.
**Warning signs:** SHA-256 hash differs between runs on unchanged files

## Voyage AI API Reference

### Endpoint
`POST https://api.voyageai.com/v1/embeddings`

### Authentication
`Authorization: Bearer $VOYAGE_API_KEY`

### Request Body
```json
{
  "input": ["text1", "text2", ...],
  "model": "voyage-3.5",
  "input_type": "document",
  "truncation": true
}
```

| Parameter | Type | Required | Default | Notes |
|-----------|------|----------|---------|-------|
| input | string or string[] | Yes | -- | Max 1000 items per request |
| model | string | Yes | -- | See model table below |
| input_type | string | No | null | "document" for indexing, "query" for search |
| truncation | boolean | No | true | Truncate texts exceeding context length |
| output_dimension | integer | No | null | 256, 512, 1024, 2048 (model-dependent) |

### Model Recommendations

| Model | Dimensions | Context | Token Limit/Request | Use Case |
|-------|-----------|---------|---------------------|----------|
| voyage-3.5 | 1024 (default) | 32K tokens | 320K tokens | **Recommended for this project** -- good quality, high throughput |
| voyage-4 | 1024 (default) | 32K tokens | 320K tokens | Newer, slightly better quality |
| voyage-4-large | 1024 (default) | 32K tokens | 120K tokens | Best quality, lower throughput |
| voyage-4-lite | 1024 (default) | 32K tokens | 1M tokens | Fastest, lowest cost |

**Recommendation:** Use `voyage-3.5` as the default model. 1024 dimensions, 320K tokens per request. Good balance of quality and throughput. The model name should be configurable via env var (e.g., `VOYAGE_MODEL`) with `voyage-3.5` as the default.

### Response Body
```json
{
  "object": "list",
  "data": [
    { "object": "embedding", "embedding": [0.1, -0.2, ...], "index": 0 }
  ],
  "model": "voyage-3.5",
  "usage": { "total_tokens": 42 }
}
```

### Rate Limits (Tier 1)
- 2000 RPM (requests per minute)
- 8M TPM (tokens per minute)
- Rate limit errors return HTTP 429

### Error Handling
- 4xx: Client errors (bad request, auth failure, rate limit)
- 5xx: Server errors (transient, should retry)
- Rate-limited (429): Retry with backoff
- Auth failure (401/403): Fail immediately with credential error

## Code Examples

### VoyageEmbedder Implementation
```rust
// Source: Voyage AI API docs (https://docs.voyageai.com/reference/embeddings-api)
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct VoyageRequest {
    input: Vec<String>,
    model: String,
    input_type: Option<String>,
    truncation: bool,
}

#[derive(Deserialize)]
struct VoyageResponse {
    data: Vec<VoyageEmbedding>,
    model: String,
    usage: VoyageUsage,
}

#[derive(Deserialize)]
struct VoyageEmbedding {
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Deserialize)]
struct VoyageUsage {
    total_tokens: u64,
}

impl VoyageEmbedder {
    pub fn new(api_key: String, model: String, dimensions: usize) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("failed to build HTTP client");
        Self { client, api_key, model, dimensions }
    }

    // embed() implementation sends POST to Voyage API
    // with Authorization: Bearer header
    // Returns EmbeddingResult with Vec<Vec<f32>>
}
```

### LanceDB Table Creation with Full Schema
```rust
// Source: LanceDB docs (https://docs.rs/lancedb/latest/lancedb/)
use std::sync::Arc;
use arrow_array::{
    ArrayRef, FixedSizeListArray, RecordBatch, StringArray, UInt32Array,
    types::Float32Type,
};
use arrow_schema::{DataType, Field, Schema};
use lancedb::connect;

const EMBEDDING_DIM: i32 = 1024;

async fn create_chunks_table(db_path: &str) -> anyhow::Result<lancedb::Table> {
    let db = connect(db_path).execute().await?;

    let schema = Arc::new(Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new("file_path", DataType::Utf8, false),
        Field::new("heading_breadcrumb", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
        Field::new("line_start", DataType::UInt32, false),
        Field::new("line_end", DataType::UInt32, false),
        Field::new("frontmatter_json", DataType::Utf8, true),
        Field::new("content_hash", DataType::Utf8, false),
        Field::new("embedding_model", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIM,
            ),
            false,
        ),
    ]));

    // Create empty table with schema, or open existing
    match db.open_table("chunks").execute().await {
        Ok(table) => Ok(table),
        Err(_) => Ok(db.create_empty_table("chunks", schema).execute().await?),
    }
}
```

### Building a RecordBatch from Chunks + Embeddings
```rust
// Source: Arrow + LanceDB examples
fn build_record_batch(
    chunks: &[Chunk],
    embeddings: &[Vec<f32>],
    hashes: &[String],
    model_id: &str,
    schema: Arc<Schema>,
) -> anyhow::Result<RecordBatch> {
    let chunk_ids: Vec<String> = chunks.iter()
        .map(|_| uuid::Uuid::new_v4().to_string())
        .collect();
    let file_paths: Vec<String> = chunks.iter()
        .map(|c| c.file_path.to_string_lossy().to_string())
        .collect();
    // ... build all arrays ...

    let vector_array = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        embeddings.iter().map(|e| Some(e.iter().map(|v| Some(*v)).collect::<Vec<_>>())),
        EMBEDDING_DIM,
    );

    Ok(RecordBatch::try_new(schema, vec![
        Arc::new(StringArray::from(chunk_ids)),
        Arc::new(StringArray::from(file_paths)),
        // ... other columns ...
        Arc::new(vector_array),
    ])?)
}
```

### indicatif Progress Bar with TTY Detection
```rust
// Source: indicatif docs (https://docs.rs/indicatif/latest/indicatif/)
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;

fn create_progress(total: u64) -> ProgressBar {
    if std::io::stdout().is_terminal() {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files ({msg})")
                .unwrap()
                .progress_chars("=>-")
        );
        pb
    } else {
        // Hidden progress bar -- no terminal output
        ProgressBar::hidden()
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Anthropic embeddings | Voyage AI embeddings | 2025 | Anthropic recommends Voyage AI for embeddings; no native Anthropic embedding endpoint |
| voyage-2 model | voyage-3.5 / voyage-4 | Jan 2026 | voyage-4 family released Jan 2026; voyage-3.5 is stable and recommended |
| lancedb 0.13 (CLAUDE.md) | lancedb 0.26.2 | Feb 2026 | Major API evolution; 0.26.2 is latest compatible with rustc 1.89 |
| arrow 53 (CLAUDE.md) | arrow 57.x | 2025-2026 | Must match lancedb 0.26.2 dependency |

**Deprecated/outdated from CLAUDE.md:**
- `lancedb ^0.13` -- now at 0.26.2 (0.27.2 latest but needs newer rustc)
- `arrow ^53.0` -- now at 57.x to match lancedb 0.26.2
- `reqwest ^0.12` -- 0.13.2 is available but 0.12 is fine and matches lancedb's internal dep
- CLAUDE.md mentions "Anthropic API" for embeddings -- project has switched to Voyage AI per CONTEXT.md

## Open Questions

1. **Frontmatter HashMap determinism**
   - What we know: `HashMap<String, serde_yml::Value>` has non-deterministic iteration order
   - What's unclear: Whether serde_json serialization of HashMap produces deterministic output (it does for string keys, but serde_yml::Value ordering within nested structures is uncertain)
   - Recommendation: Change `extra` field in Frontmatter to `BTreeMap<String, serde_yml::Value>` for guaranteed deterministic serialization. This is a minor change to types.rs.

2. **Batch size for Voyage AI**
   - What we know: Max 1000 items per request, 320K tokens per request for voyage-3.5
   - What's unclear: Optimal batch size balancing throughput vs. memory vs. error blast radius
   - Recommendation: Start with 50 chunks per batch. Each markdown chunk is typically 200-500 tokens. 50 chunks ~ 10K-25K tokens, well under the 320K limit. Smaller batches mean smaller blast radius on failure.

3. **LanceDB table-per-vault vs. single table**
   - What we know: Phase 2 is single-vault (`local-index index <path>`)
   - What's unclear: Whether to use a single "chunks" table or partition by something
   - Recommendation: Single "chunks" table. file_path is vault-relative, which is sufficient for all queries. Multiple vaults use separate --data-dir values (separate LanceDB instances).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustc | Compilation | Yes | 1.89.0-nightly | -- |
| cargo | Build | Yes | (bundled with rustc) | -- |
| Voyage AI API | Embedding | External service | -- | None (required) |
| Network access | API calls | Required at index time | -- | None |

**Missing dependencies with no fallback:**
- Voyage AI API key (`VOYAGE_API_KEY`) must be set by operator

**Missing dependencies with fallback:**
- None

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + tokio::test for async |
| Config file | None needed (Cargo.toml [dev-dependencies]) |
| Quick run command | `cargo test` |
| Full suite command | `cargo test -- --include-ignored` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-01 | index command runs end-to-end with embeddings | integration | `cargo test --test index_integration` | Exists (Phase 1, needs update) |
| CRED-01 | VOYAGE_API_KEY env var resolution | unit | `cargo test credentials` | Wave 0 |
| CRED-02 | Embedder trait + VoyageEmbedder | unit | `cargo test embedder` | Wave 0 |
| CRED-03 | Startup fails without credentials | integration | `cargo test --test index_integration::no_credentials` | Wave 0 |
| INDX-04 | SHA-256 hash skip unchanged chunks | unit | `cargo test content_hash` | Wave 0 |
| INDX-05 | LanceDB schema stores all metadata | unit | `cargo test store` | Wave 0 |
| INDX-06 | Model mismatch guard | unit | `cargo test model_mismatch` | Wave 0 |
| INDX-07 | Exponential backoff on transient errors | unit (with wiremock) | `cargo test retry` | Wave 0 |
| INDX-08 | Progress reporting (TTY vs non-TTY) | integration | `cargo test --test index_integration::progress_output` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test`
- **Per wave merge:** `cargo test -- --include-ignored`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/credentials_test.rs` or `src/credentials.rs` unit tests -- covers CRED-01, CRED-03
- [ ] `src/pipeline/embedder.rs` unit tests -- covers CRED-02 (trait design, VoyageEmbedder with wiremock)
- [ ] `src/pipeline/store.rs` unit tests -- covers INDX-04, INDX-05, INDX-06 (LanceDB operations with temp dirs)
- [ ] Update `tests/index_integration.rs` -- covers CLI-01 end-to-end with mocked embeddings
- [ ] Add `wiremock` to dev-dependencies for mocking Voyage AI API
- [ ] Add `tempfile` already present in dev-dependencies (reuse for LanceDB temp dirs)

## Project Constraints (from CLAUDE.md)

- **Tech stack:** Rust only -- no Node/Python helpers
- **Embeddings:** Voyage AI only in Phase 2 (was "Anthropic API" in CLAUDE.md, overridden by CONTEXT.md decisions)
- **Database:** LanceDB embedded -- no external database process
- **CLI framework:** clap with derive macros
- **Logging:** tracing crate (no log crate directly)
- **Error handling:** anyhow at application layer, thiserror for library error enums
- **Patterns:** Per-file graceful error handling: warn and continue, never abort entire walk

## Sources

### Primary (HIGH confidence)
- [Voyage AI Embeddings API Reference](https://docs.voyageai.com/reference/embeddings-api) -- endpoint, request/response format, models, parameters
- [Voyage AI Embeddings Introduction](https://docs.voyageai.com/docs/embeddings) -- model dimensions, context lengths
- [LanceDB Rust docs (docs.rs)](https://docs.rs/lancedb/latest/lancedb/) -- Connection, Table, Query APIs
- [LanceDB simple.rs example](https://github.com/lancedb/lancedb/blob/main/rust/lancedb/examples/simple.rs) -- RecordBatch creation, schema, vector search
- [crates.io: lancedb](https://crates.io/crates/lancedb) -- version 0.27.2 (latest), 0.26.2 (compatible)
- cargo search / cargo add --dry-run -- version verification (lancedb 0.26.2, arrow 57.x, indicatif 0.18.4, sha2 0.11.0)

### Secondary (MEDIUM confidence)
- [Voyage AI Rate Limits](https://docs.voyageai.com/docs/rate-limits) -- 2000 RPM, 8M TPM at tier 1
- [LanceDB FTS documentation](https://docs.lancedb.com/search/full-text-search) -- FTS is available in Rust via `Index::FTS`
- [indicatif docs](https://docs.rs/indicatif/latest/indicatif/) -- ProgressBar API, TTY auto-detection

### Tertiary (LOW confidence)
- Arrow version constraint (^57.2) for lancedb 0.26.2 -- inferred from docs.rs metadata for 0.26.2. Cargo resolution will validate this at build time.

## Metadata

**Confidence breakdown:**
- Standard stack: MEDIUM -- lancedb version pinning verified via cargo add --dry-run; arrow version from docs.rs metadata
- Architecture: HIGH -- patterns follow official LanceDB examples and Voyage AI docs
- Pitfalls: HIGH -- arrow version mismatch and rustc MSRV are verified real risks
- Voyage AI API: HIGH -- directly from official API reference docs

**Research date:** 2026-04-09
**Valid until:** 2026-04-23 (14 days -- lancedb evolves rapidly; rustc nightly may update)
