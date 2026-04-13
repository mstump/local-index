# local-index

A Rust daemon that watches a directory tree (initially an Obsidian vault), chunks markdown files by heading, embeds each chunk via the Voyage AI API, and stores everything in an embedded LanceDB database. Exposes full-text and semantic search via CLI, a Claude Code skill interface, and a web dashboard — enabling Claude to reason over your notes without resorting to grep.

## Features

- **Semantic search** — vector similarity via Voyage AI embeddings (voyage-3.5, 1024 dims)
- **Full-text search** — BM25 via LanceDB's native FTS engine
- **Hybrid search** — fuses BM25 + vector results via Reciprocal Rank Fusion (RRF)
- **Heading-aware chunking** — chunks by heading hierarchy with overlap for context preservation
- **Web dashboard** — browse indexed files, run searches, view index status
- **Claude Code skills** — invoke search, reindex, and status directly from Claude

## Quick Start

```sh
cargo install local-index
export VOYAGE_API_KEY="your-key-here"
export OBSIDIAN_VAULT="/path/to/your/vault"
local-index index "$OBSIDIAN_VAULT"
local-index search "your query"
```

## CLI Reference

### Index

```sh
local-index index <PATH> [--force-reindex] [--data-dir PATH]
```

Walk a directory recursively, chunk all `.md` files by heading, embed each chunk, and store in LanceDB.

### Search

```sh
local-index search "<QUERY>" [--limit N] [--min-score F] [--mode semantic|fts|hybrid] \
  [--path-filter PATH_PREFIX] [--tag-filter TAG] [--context N] [--format json|pretty]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--limit` / `-n` | 10 | Maximum results |
| `--min-score` | (none) | Score threshold 0.0–1.0 |
| `--mode` | hybrid | Search strategy |
| `--path-filter` | (none) | Restrict to path prefix |
| `--tag-filter` | (none) | Restrict by frontmatter tag |
| `--context` | 0 | Surrounding chunks to include |
| `--format` | json | Output format |

### Status

```sh
local-index status [--data-dir PATH]
```

Show total chunks, files indexed, last index time, and embedding model info.

### Daemon

```sh
local-index daemon <PATH> [--bind ADDR]
```

Watch a directory for changes and re-index automatically. Also serves the web dashboard.

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `VOYAGE_API_KEY` | Yes (for index/search) | Voyage AI API key |
| `LOCAL_INDEX_DATA_DIR` | No | Override default data directory |
| `LOCAL_INDEX_BIND` | No | HTTP bind address (default: `127.0.0.1:3000`) |
| `LOCAL_INDEX_LOG_LEVEL` | No | Log level (default: `info`) |
| `OBSIDIAN_VAULT` | Conventional | Vault path used by Claude Code reindex skill |
| `LOCAL_INDEX_VAULT` | Conventional | Alternative vault path for Claude Code skill |

## Claude Code Integration

This repository ships with Claude Code skill files so Claude can search your vault, trigger reindexes, and check index status without any manual intervention.

### Installation

```sh
cargo install local-index
# Verify
local-index --version
```

### Required Environment Variables

Add these to `~/.zshrc`, `~/.bashrc`, or `~/.config/fish/config.fish`:

```sh
export VOYAGE_API_KEY="your-key-here"        # Required for search and indexing
export OBSIDIAN_VAULT="/path/to/your/vault"  # Conventional; used by the reindex skill
export LOCAL_INDEX_DATA_DIR="/path/to/data"  # Optional; defaults to platform data dir
```

### Index Your Vault

```sh
local-index index "$OBSIDIAN_VAULT"
```

### Skills Setup

The three skill files are already present in `.claude/skills/` in this repository.
Claude Code will automatically discover them when this repo is the working directory.

Skill files:

| File | Purpose |
|------|---------|
| `.claude/skills/search.md` | Semantic, hybrid, and full-text search |
| `.claude/skills/reindex.md` | One-shot reindex trigger with env var path strategy |
| `.claude/skills/status.md` | Index health check (no API key required) |

### Shell Wrappers (Optional)

The `scripts/` directory contains thin pass-through wrappers. These are optional
conveniences — the skill files call `local-index` directly.

```sh
scripts/search.sh "your query" --limit 5
scripts/reindex.sh "$OBSIDIAN_VAULT"
scripts/status.sh
```

## Building from Source

```sh
git clone https://github.com/you/local-index
cd local-index
cargo build --release
```

## License

MIT
