# Phase 1: Foundation & File Processing - Research

**Researched:** 2026-04-08
**Domain:** Rust CLI scaffolding, structured logging, markdown parsing/chunking, YAML frontmatter extraction
**Confidence:** HIGH (Phase 1 is pure Rust logic with no external API calls -- all crates are mature and well-documented)

## Summary

Phase 1 builds the non-networked foundation of local-index: a clap-based CLI skeleton with all five subcommands stubbed, structured logging via tracing, .env loading via dotenvy, recursive directory traversal, markdown chunking by heading, and YAML frontmatter extraction. There are zero external service dependencies -- no LanceDB, no embedding API, no HTTP server. The output is a binary that can discover markdown files, parse them into structured chunks, and log what it found.

The core technical challenge is the markdown chunker: splitting a file by headings while preserving heading hierarchy as breadcrumbs (`## Goals > ### Q1`), stripping YAML frontmatter into structured metadata, and handling edge cases (no headings, empty sections, nested headings, Obsidian wiki-links). pulldown-cmark 0.13.3 provides native support for both heading events and YAML metadata block detection, making it the ideal single-crate solution for parsing.

**Primary recommendation:** Use pulldown-cmark with `ENABLE_YAML_STYLE_METADATA_BLOCKS` and `ENABLE_HEADING_ATTRIBUTES` options for a single-pass parse that extracts both frontmatter and heading structure. Use `serde_yml` (not the deprecated `serde_yaml`) for YAML deserialization of frontmatter. Structure the chunker as a pure function `fn chunk_markdown(content: &str, file_path: &Path) -> ChunkResult` that is trivially unit-testable.

## Project Constraints (from CLAUDE.md)

- **Tech stack**: Rust only -- no Node/Python helpers
- **CLI framework**: `clap` with derive macros
- **Logging**: `tracing` crate (no `log` crate directly)
- **Deployment**: Single binary, runs on macOS (primary), Linux (secondary)
- **GSD Workflow**: All repo edits through GSD commands unless user explicitly bypasses

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-06 | All CLI commands/flags with `clap` derive macros, useful `--help` with examples | Clap 4.6 derive macros with `env` feature; single `Cli` struct with `Command` enum for 5 subcommands |
| CLI-07 | All settings via CLI flags, `.env` file, and env vars -- no config file required | dotenvy 0.15.7 loaded before clap parse; clap `#[arg(env = "...")]` for env var fallback |
| CLI-08 | Structured logging via `tracing`; level via `RUST_LOG` or `--log-level` | tracing 0.1 + tracing-subscriber 0.3.20 with `EnvFilter` and `fmt` layer |
| INDX-01 | Recursive directory walk of `.md` files; non-markdown skipped with trace log | walkdir 2.5.0 with `.md` extension filter; `tracing::trace!` for skipped files |
| INDX-02 | Chunk by heading with hierarchy preserved as breadcrumb (e.g., `## Goals > ### Q1`) | pulldown-cmark 0.13.3 heading events with breadcrumb stack algorithm |
| INDX-03 | YAML frontmatter stripped from chunks but available as structured metadata | pulldown-cmark `ENABLE_YAML_STYLE_METADATA_BLOCKS` + `serde_yml` for YAML parsing |
</phase_requirements>

## Standard Stack

### Core (Phase 1 only)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.6.0 | CLI argument parsing | Project requirement. Derive macros + `env` feature for env var fallback. Industry standard. |
| tracing | 0.1 | Structured logging | Project requirement. Spans + structured fields. |
| tracing-subscriber | 0.3.20 | Log formatting + filtering | `EnvFilter` for RUST_LOG, `fmt` layer for console output. |
| pulldown-cmark | 0.13.3 | Markdown parsing | Streaming pull parser. Native heading events + YAML metadata block detection. |
| serde_yml | 0.0.12 | YAML frontmatter parsing | Maintained fork of deprecated `serde_yaml`. Deserializes frontmatter into typed structs. |
| dotenvy | 0.15.7 | .env file loading | Maintained fork of `dotenv`. Loads before clap parse. |
| walkdir | 2.5.0 | Recursive directory traversal | Standard crate for recursive file discovery. Handles symlinks, depth limits. |
| serde | 1.0 | Serialization framework | Needed for frontmatter deserialization and chunk data structures. |
| serde_json | 1.0 | JSON output | For eventual CLI JSON output (stub in Phase 1). |
| anyhow | 1.0 | Application error handling | Ergonomic error chaining with context for the binary layer. |
| thiserror | 2.0.18 | Library error types | Structured error enums for chunker, parser, walker errors. |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1 (features: full) | Async runtime | Needed even in Phase 1 for async main. Sets up the runtime that later phases extend. |
| tempfile | 3.0 | Test temp directories | In dev-dependencies for chunker tests with real file trees. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| serde_yml | serde_yaml 0.9.34 | Deprecated, unmaintained. serde_yml is the active fork. |
| serde_yml | yaml-front-matter crate | Wraps serde_yaml internally; adds a dependency for something trivial. Manual split + serde_yml is simpler. |
| walkdir | jwalk | jwalk is parallel but adds complexity. walkdir is sufficient for sequential file discovery; parallelism is not needed here. |
| pulldown-cmark | comrak | comrak builds full AST (heavier). We only need streaming heading detection. pulldown-cmark's pull model is ideal. |

**Installation (Phase 1 Cargo.toml dependencies):**
```bash
cargo add clap --features derive,env
cargo add tracing
cargo add tracing-subscriber --features env-filter,fmt,json
cargo add pulldown-cmark
cargo add serde_yml
cargo add dotenvy
cargo add walkdir
cargo add serde --features derive
cargo add serde_json
cargo add anyhow
cargo add thiserror
cargo add tokio --features full
cargo add --dev tempfile
```

## Architecture Patterns

### Recommended Project Structure (Phase 1 scope)

```
src/
  main.rs                # Entry point: dotenvy::dotenv() -> clap parse -> tracing init -> dispatch
  cli.rs                 # Clap derive structs: Cli, Command enum, per-subcommand args
  logging.rs             # tracing-subscriber setup: EnvFilter + fmt layer
  pipeline/
    mod.rs               # Re-exports
    walker.rs            # walkdir-based recursive .md file discovery
    chunker.rs           # Markdown -> Vec<Chunk> by heading, frontmatter extraction
  types.rs               # Chunk, Frontmatter, FileInfo data structures
  error.rs               # thiserror error enums: ChunkError, WalkError, ConfigError
```

**Note:** Later phases will add `pipeline/embedder.rs`, `pipeline/coordinator.rs`, `store/`, `server/`, etc. Phase 1 keeps the structure minimal but in the right namespace so future additions are clean.

### Pattern 1: Initialization Order

**What:** The startup sequence must follow a strict order to ensure .env values are available to clap's env var resolution.

**When to use:** Always -- this is the `main()` function.

**Example:**
```rust
// src/main.rs
use anyhow::Result;
use clap::Parser;

mod cli;
mod logging;
mod pipeline;
mod types;
mod error;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load .env FIRST (before clap parses, so env vars are populated)
    dotenvy::dotenv().ok(); // .ok() ignores missing .env file

    // 2. Parse CLI args (clap reads env vars via #[arg(env = "...")])
    let cli = cli::Cli::parse();

    // 3. Initialize tracing (uses --log-level from CLI or RUST_LOG from env)
    logging::init(&cli.log_level)?;

    // 4. Dispatch to subcommand handler
    match cli.command {
        cli::Command::Index(args) => { /* Phase 1: walk + chunk + print */ }
        cli::Command::Daemon(_) => { todo!("Phase 4") }
        cli::Command::Search(_) => { todo!("Phase 3") }
        cli::Command::Status => { todo!("Phase 4") }
        cli::Command::Serve(_) => { todo!("Phase 5") }
    }

    Ok(())
}
```

### Pattern 2: Clap Derive with Global Flags + Subcommands

**What:** Single `Cli` struct with global flags (vault-path, db-path, log-level) and a `Command` enum for subcommands.

**Example:**
```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "local-index",
    about = "Semantic search over local markdown vaults",
    version,
    long_about = None,
)]
pub struct Cli {
    /// Path to the vault directory to index
    #[arg(long, env = "LOCAL_INDEX_VAULT_PATH", default_value = ".")]
    pub vault_path: PathBuf,

    /// Path to the LanceDB database directory
    #[arg(long, env = "LOCAL_INDEX_DB_PATH", default_value = ".local-index")]
    pub db_path: PathBuf,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "LOCAL_INDEX_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Index all markdown files in the vault (one-shot)
    Index(IndexArgs),
    /// Start daemon: watch files + serve HTTP
    Daemon(DaemonArgs),
    /// Search the index
    Search(SearchArgs),
    /// Show index status
    Status,
    /// Start HTTP server only (no file watcher)
    Serve(ServeArgs),
}

#[derive(clap::Args, Debug)]
pub struct IndexArgs {
    /// Force re-index all chunks, ignoring content hashes
    #[arg(long)]
    pub force: bool,
}

// Stubs for other subcommands...
#[derive(clap::Args, Debug)]
pub struct DaemonArgs {
    #[arg(long, env = "LOCAL_INDEX_PORT", default_value_t = 3000)]
    pub port: u16,
    #[arg(long, env = "LOCAL_INDEX_BIND", default_value = "127.0.0.1")]
    pub bind: String,
}

#[derive(clap::Args, Debug)]
pub struct SearchArgs {
    /// Search query text
    pub query: String,
    #[arg(short = 'n', long, default_value_t = 10)]
    pub limit: usize,
}

#[derive(clap::Args, Debug)]
pub struct ServeArgs {
    #[arg(long, env = "LOCAL_INDEX_PORT", default_value_t = 3000)]
    pub port: u16,
    #[arg(long, env = "LOCAL_INDEX_BIND", default_value = "127.0.0.1")]
    pub bind: String,
}
```

**Key details:**
- `env = "LOCAL_INDEX_..."` prefix avoids collisions with other tools
- `default_value` provides sane defaults -- no config file needed for basic operation
- Precedence is: CLI flag > env var > default (clap handles this automatically)

### Pattern 3: Tracing Initialization with EnvFilter

**What:** Set up tracing-subscriber with `EnvFilter` that respects both `--log-level` and `RUST_LOG`.

**Example:**
```rust
use anyhow::Result;
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init(log_level: &str) -> Result<()> {
    // RUST_LOG takes precedence if set; otherwise use --log-level value
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true).with_thread_ids(false))
        .init();

    Ok(())
}
```

**Important:** Call `tracing_subscriber::registry().init()` exactly once, in `main()`, before any other code. Setting a global subscriber twice panics.

### Pattern 4: Heading Chunking Algorithm with Breadcrumb Stack

**What:** Iterate pulldown-cmark events, maintain a stack of active headings, accumulate text between headings into chunks.

**Algorithm:**
1. Parse with `ENABLE_YAML_STYLE_METADATA_BLOCKS | ENABLE_HEADING_ATTRIBUTES`
2. On `Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle))` -- enter frontmatter mode, collect text
3. On `Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle))` -- parse collected YAML
4. On `Event::Start(Tag::Heading { level, .. })` -- finalize current chunk, update heading stack
5. On `Event::Text(text)` / `Event::Code(code)` -- append to current chunk body
6. On `Event::End(TagEnd::Heading(level))` -- heading text collection complete
7. At EOF -- finalize last chunk

**Heading stack management:** Maintain `Vec<(HeadingLevel, String)>`. When a new heading arrives at level N, pop all entries with level >= N, then push the new heading. The breadcrumb is the remaining stack joined with ` > `.

```rust
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd, MetadataBlockKind};

fn update_heading_stack(
    stack: &mut Vec<(HeadingLevel, String)>,
    level: HeadingLevel,
    text: String,
) {
    // Pop headings at same or deeper level
    while stack.last().map_or(false, |(l, _)| *l >= level) {
        stack.pop();
    }
    stack.push((level, text));
}

fn breadcrumb(stack: &[(HeadingLevel, String)]) -> String {
    stack.iter()
        .map(|(level, text)| {
            let prefix = "#".repeat(*level as usize);
            format!("{} {}", prefix, text)
        })
        .collect::<Vec<_>>()
        .join(" > ")
}
```

### Anti-Patterns to Avoid

- **Parsing frontmatter with regex:** YAML frontmatter can contain `---` inside multi-line strings. Use pulldown-cmark's native metadata block detection instead.
- **Using `serde_yaml`:** Deprecated. Use `serde_yml` instead.
- **Setting tracing subscriber in a library function:** Always set it in `main()`. Setting it twice panics.
- **Loading dotenvy after clap parse:** .env values won't be available for clap's `env` attribute resolution.
- **Using `Options::all()` in pulldown-cmark:** Only enable the options you need. `Options::all()` enables experimental features that may change behavior.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| YAML frontmatter detection | Regex-based `---` delimiter splitting | pulldown-cmark `ENABLE_YAML_STYLE_METADATA_BLOCKS` | Regex breaks on `---` inside YAML strings; pulldown-cmark handles edge cases correctly |
| YAML parsing | Manual key-value extraction | `serde_yml` with `#[derive(Deserialize)]` struct | YAML is complex (anchors, multi-line, type coercion); serde handles it correctly |
| Recursive directory walk | Manual `std::fs::read_dir` recursion | `walkdir` crate | Handles symlink loops, permission errors, depth limits |
| CLI argument parsing | Manual `std::env::args` parsing | `clap` derive macros | Help text generation, env var fallback, type validation, shell completions |
| Log level filtering | Manual if-statements around log calls | `tracing` `EnvFilter` | Dynamic filtering by module, span, target; no code changes needed |

## Common Pitfalls

### Pitfall 1: dotenvy Must Load Before clap::Parser::parse()

**What goes wrong:** If you call `Cli::parse()` before `dotenvy::dotenv()`, clap's `#[arg(env = "...")]` won't see values from `.env` because they haven't been loaded into the process environment yet.
**Why it happens:** Both are called in `main()` and the ordering seems unimportant.
**How to avoid:** Always: `dotenvy::dotenv().ok()` first, then `Cli::parse()`. The `.ok()` is important -- it ignores "no .env file found" which is the normal case in production.
**Warning signs:** Settings from `.env` are ignored; only CLI flags and actual env vars work.

### Pitfall 2: Frontmatter-Only Files Produce Zero Chunks

**What goes wrong:** A markdown file with only YAML frontmatter and no headings or body text produces no chunks. The chunker silently returns an empty vec.
**Why it happens:** No heading events fire, so no chunks are created.
**How to avoid:** If the file has content after frontmatter but no headings, create a single chunk with the entire body and an empty breadcrumb. Track and log "files with zero chunks" for debugging.
**Warning signs:** Files that exist on disk but have zero chunks in the index.

### Pitfall 3: Files with No Headings

**What goes wrong:** A plain markdown file with no headings (just paragraphs) produces zero chunks if the chunker only creates chunks on heading events.
**Why it happens:** The algorithm waits for a heading to start a chunk, but no heading ever arrives.
**How to avoid:** Initialize a "pre-heading" chunk that accumulates any content before the first heading. If the file has no headings at all, emit this single chunk with an empty heading breadcrumb and the file name as context.
**Warning signs:** Same as Pitfall 2 -- files with zero chunks.

### Pitfall 4: Heading Text Spanning Multiple Events

**What goes wrong:** A heading like `## Hello **world**` produces multiple events: `Start(Heading)`, `Text("Hello ")`, `Start(Strong)`, `Text("world")`, `End(Strong)`, `End(Heading)`. If you only capture the first `Text` event, you lose "world".
**Why it happens:** pulldown-cmark emits inline formatting as nested events within heading start/end.
**How to avoid:** Track an "in-heading" state. Between `Start(Heading)` and `End(Heading)`, accumulate all `Text` and `Code` events into the heading text buffer. Ignore formatting tags (bold, italic, etc.) but keep the text content.
**Warning signs:** Heading breadcrumbs truncated or missing words.

### Pitfall 5: Obsidian Wiki-Links in Chunk Content

**What goes wrong:** `[[note-name]]` and `[[note|display]]` syntax is not standard markdown. pulldown-cmark passes it through as text. These brackets add noise to embeddings in later phases.
**Why it happens:** pulldown-cmark is a CommonMark parser; Obsidian extensions are not in spec.
**How to avoid:** Post-process chunk text with a simple regex pass: `[[target|display]]` -> `display`, `[[target]]` -> `target`, `![[embed]]` -> remove entirely. Do this after pulldown-cmark parsing, not before. Store raw wiki-link targets as metadata for future cross-reference features (v2 LINK-01).
**Warning signs:** Search results in later phases containing literal `[[` and `]]` brackets.

### Pitfall 6: walkdir Symlink Handling

**What goes wrong:** Following symlinks can cause infinite loops if a symlink points to a parent directory. walkdir defaults to not following symlinks.
**Why it happens:** Users symlink shared note directories into their vault.
**How to avoid:** Keep walkdir's default `follow_links(false)`. If symlink support is needed later, enable it with `contents_first(true)` and the built-in loop detection that walkdir provides. For Phase 1, document that symlinks are not followed.
**Warning signs:** 100% CPU during directory scan, or "too many open files" errors.

## Code Examples

### Complete Chunk Data Structure

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;

/// Parsed YAML frontmatter from a markdown file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    /// Catch-all for unknown frontmatter fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yml::Value>,
}

/// A single chunk extracted from a markdown file
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// Vault-relative file path
    pub file_path: PathBuf,
    /// Heading hierarchy breadcrumb (e.g., "## Goals > ### Q1")
    pub heading_breadcrumb: String,
    /// The heading level of this chunk's immediate heading (0 = pre-heading content)
    pub heading_level: u8,
    /// The chunk's text content (frontmatter stripped, headings stripped)
    pub body: String,
    /// Start line number in the source file (1-based)
    pub line_start: usize,
    /// End line number in the source file (1-based)
    pub line_end: usize,
    /// Frontmatter metadata from the file (shared across all chunks from same file)
    pub frontmatter: Frontmatter,
}

/// Result of chunking a single file
#[derive(Debug)]
pub struct ChunkedFile {
    pub file_path: PathBuf,
    pub frontmatter: Frontmatter,
    pub chunks: Vec<Chunk>,
}
```

**Design notes for Phase 2 compatibility:**
- `file_path` is vault-relative (strip the vault root prefix before storing)
- `heading_breadcrumb` is the display string, not a structured hierarchy -- simpler to store and search
- `body` is the clean text that will be embedded (no frontmatter, no heading text)
- `line_start` / `line_end` enable jumping to the exact location in the editor
- `frontmatter` is shared across all chunks from the same file (stored per-chunk for denormalization in LanceDB)

### Walker with .md Filter and Trace Logging

```rust
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use tracing::{debug, trace, info};

pub fn discover_markdown_files(vault_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in WalkDir::new(vault_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
    {
        match entry {
            Ok(entry) => {
                if entry.file_type().is_file() {
                    if entry.path().extension().map_or(false, |ext| ext == "md") {
                        debug!(path = %entry.path().display(), "discovered markdown file");
                        files.push(entry.into_path());
                    } else {
                        trace!(
                            path = %entry.path().display(),
                            "skipping non-markdown file"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "error walking directory");
            }
        }
    }

    info!(count = files.len(), "markdown file discovery complete");
    files
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map_or(false, |s| s.starts_with('.'))
}
```

### Frontmatter Extraction via pulldown-cmark

```rust
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, MetadataBlockKind};

pub fn extract_frontmatter(content: &str) -> (Option<Frontmatter>, usize) {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

    let parser = Parser::new_ext(content, options);
    let mut yaml_text = String::new();
    let mut in_metadata = false;
    let mut end_offset = 0;

    for (event, range) in Parser::new_ext(content, options).into_offset_iter() {
        match event {
            Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                in_metadata = true;
            }
            Event::Text(text) if in_metadata => {
                yaml_text.push_str(&text);
            }
            Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                end_offset = range.end;
                break;
            }
            _ => {
                if !in_metadata {
                    break; // No frontmatter at start of file
                }
            }
        }
    }

    if yaml_text.is_empty() {
        return (None, 0);
    }

    match serde_yml::from_str::<Frontmatter>(&yaml_text) {
        Ok(fm) => (Some(fm), end_offset),
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse YAML frontmatter; treating as content");
            (None, 0)
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `serde_yaml` for YAML parsing | `serde_yml` (maintained fork) | 2024 (serde_yaml deprecated) | Must use serde_yml; serde_yaml is unmaintained |
| Manual frontmatter regex split | pulldown-cmark `ENABLE_YAML_STYLE_METADATA_BLOCKS` | pulldown-cmark 0.10+ | Parser handles delimiter detection correctly |
| `dotenv` crate | `dotenvy` crate | 2022 (dotenv unmaintained) | Use dotenvy 0.15.7 |
| clap 3.x builder pattern | clap 4.x derive macros with `env` feature | 2023 | Derive macros are the standard approach |
| `thiserror` 1.x | `thiserror` 2.0 | 2024 | 2.0 released; use it |

## Open Questions

1. **serde_yml maturity and version stability**
   - What we know: It is a fork of serde_yaml by sebastienrousseau, appears maintained
   - What's unclear: How stable the API is at version 0.0.12 (pre-1.0). Whether it tracks serde_yaml's API exactly.
   - Recommendation: Pin the exact version in Cargo.toml. If serde_yml proves problematic, fall back to `serde_yaml 0.9.34` (deprecated but functional) or manual YAML parsing.

2. **pulldown-cmark MetadataBlock offset tracking**
   - What we know: `into_offset_iter()` provides byte offsets for each event
   - What's unclear: Whether the offset after `End(MetadataBlock)` correctly marks where content begins (after the closing `---`)
   - Recommendation: Write a unit test that verifies offset positioning with a known frontmatter+content file. If offsets are wrong, fall back to manual `---` delimiter detection.

3. **Obsidian-specific heading syntax**
   - What we know: Obsidian supports standard markdown headings. Some users put wiki-links in headings: `## [[Project Name]] Goals`
   - What's unclear: How common this pattern is and whether it should be normalized in breadcrumbs
   - Recommendation: For Phase 1, preserve wiki-link syntax in breadcrumbs as-is. Post-processing wiki-links in breadcrumbs can be a follow-up.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Everything | Yes | 1.89.0-nightly | -- |
| cargo | Build system | Yes | 1.89.0-nightly | -- |

No external services, databases, or tools required for Phase 1. All dependencies are Rust crates resolved via cargo.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (`#[cfg(test)]` + `#[test]`) |
| Config file | None needed (Rust's test framework is zero-config) |
| Quick run command | `cargo test` |
| Full suite command | `cargo test -- --include-ignored` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-06 | `--help` output lists all subcommands | integration | `cargo test --test cli_tests::test_help_output` | Wave 0 |
| CLI-07 | .env values override defaults, CLI flags override .env | unit | `cargo test cli::tests::test_config_precedence` | Wave 0 |
| CLI-08 | `--log-level debug` produces debug output; RUST_LOG overrides | unit | `cargo test logging::tests::test_log_level` | Wave 0 |
| INDX-01 | Recursive .md discovery, non-.md skipped with trace | unit | `cargo test pipeline::walker::tests::test_discover_md_files` | Wave 0 |
| INDX-02 | Heading chunking with breadcrumbs | unit | `cargo test pipeline::chunker::tests::test_heading_chunking` | Wave 0 |
| INDX-03 | Frontmatter stripped from content, parsed as metadata | unit | `cargo test pipeline::chunker::tests::test_frontmatter_extraction` | Wave 0 |

### Chunker Edge Case Tests (critical)

These are the most important unit tests in Phase 1:

| Test Case | Input | Expected Output |
|-----------|-------|----------------|
| No headings | Plain paragraphs, no `#` lines | Single chunk with empty breadcrumb |
| Empty sections | `## A\n## B` (heading with no body) | Skip empty chunk or emit with empty body |
| Nested headings | `# A\n## B\n### C\n## D` | Breadcrumb for C is `# A > ## B > ### C`; D resets to `# A > ## D` |
| Frontmatter only | `---\ntags: [test]\n---\n` | Zero chunks, frontmatter parsed |
| Frontmatter + content | `---\ntags: [test]\n---\n# Title\nBody` | One chunk, frontmatter available on chunk |
| Malformed frontmatter | `---\n{invalid yaml\n---\nContent` | Frontmatter parse fails gracefully, content still chunked |
| Wiki-links in body | `## Heading\nSee [[other note]]` | Body contains wiki-link text (post-processing optional) |
| Multi-event headings | `## Hello **world**` | Heading text is "Hello world" (formatting stripped) |
| Deeply nested (H1-H6) | Six levels of heading | Breadcrumb contains all six levels |
| Content before first heading | `Some text\n# First heading\nMore text` | Two chunks: pre-heading content + heading content |

### Sampling Rate

- **Per task commit:** `cargo test`
- **Per wave merge:** `cargo test -- --include-ignored`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/cli_tests.rs` -- integration test for CLI help output (assert_cmd or manual binary invocation)
- [ ] Unit test modules in `pipeline/walker.rs`, `pipeline/chunker.rs`, `logging.rs`, `cli.rs`
- [ ] `tests/fixtures/` -- markdown fixture files for chunker edge case tests

## Sources

### Primary (HIGH confidence)
- [pulldown-cmark 0.13.3 docs](https://docs.rs/pulldown-cmark/0.13.3/) -- Tag::Heading, Tag::MetadataBlock, HeadingLevel, Options
- [clap 4.6 docs](https://docs.rs/clap/latest/clap/) -- derive macros, env feature, subcommands
- [tracing-subscriber docs](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/) -- EnvFilter, fmt layer
- [walkdir 2.5.0 docs](https://docs.rs/walkdir/latest/walkdir/) -- WalkDir, follow_links, filter_entry
- [dotenvy 0.15.7 docs](https://docs.rs/dotenvy/latest/dotenvy/) -- dotenv() function
- [crates.io](https://crates.io) -- verified version numbers for all crates

### Secondary (MEDIUM confidence)
- [serde_yml GitHub](https://github.com/sebastienrousseau/serde_yml) -- maintained fork status, API compatibility
- [Rust users forum on serde_yaml deprecation](https://users.rust-lang.org/t/serde-yaml-deprecation-alternatives/108868) -- community consensus on alternatives

### Tertiary (LOW confidence)
- pulldown-cmark `into_offset_iter()` behavior with MetadataBlock -- needs empirical validation via unit tests

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all crates are mature, versions verified against crates.io
- Architecture: HIGH -- patterns are standard Rust idioms (clap derive, tracing subscriber, walkdir)
- Chunking algorithm: MEDIUM -- the heading stack approach is well-known but edge cases need empirical testing
- Frontmatter parsing: MEDIUM -- pulldown-cmark's MetadataBlock support is documented but offset behavior needs verification
- Pitfalls: HIGH -- based on well-known Rust ecosystem patterns and pulldown-cmark documentation

**Research date:** 2026-04-08
**Valid until:** 2026-05-08 (30 days -- all dependencies are stable)
