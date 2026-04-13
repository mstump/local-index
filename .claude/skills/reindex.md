# Skill: local-index reindex

## Purpose

This skill enables Claude to trigger a one-shot full reindex of a markdown vault.
Use it when the index is missing, stale, or after changing the embedding model.
Claude follows the path decision tree below without asking the user for format guidance.

## Prerequisites

- `local-index` binary installed (`cargo install local-index`) and available in PATH
- `VOYAGE_API_KEY` environment variable set (required for embedding)
- A vault directory to index
- Optional: `LOCAL_INDEX_DATA_DIR` if using a non-default data directory

## Path Decision Tree (CRITICAL — follow in order)

Before invoking `local-index index`, Claude MUST determine the vault path using this sequence:

1. Check if `$OBSIDIAN_VAULT` is set in the environment
2. If set → use it: `local-index index "$OBSIDIAN_VAULT"`
3. If `$OBSIDIAN_VAULT` is not set → check `$LOCAL_INDEX_VAULT`
4. If `$LOCAL_INDEX_VAULT` is set → use it: `local-index index "$LOCAL_INDEX_VAULT"`
5. If neither is set → ask the user: "What path should I reindex?"

**Note:** The `local-index` binary does NOT auto-discover the vault path. It must be
provided explicitly as a positional argument. Never assume a default path.

## Invocation

```sh
local-index index <PATH> [--force-reindex] [--data-dir PATH]
```

## Flags

| Flag | Description |
|------|-------------|
| `--force-reindex` | Skip content hash check; re-embed all chunks even if unchanged |
| `--data-dir PATH` | Override index data directory (env: `LOCAL_INDEX_DATA_DIR`) |

## Example Invocations

```sh
# Standard reindex using env var (preferred)
local-index index "$OBSIDIAN_VAULT"

# Force reindex (e.g., after changing embedding model or if results seem stale)
local-index index "$OBSIDIAN_VAULT" --force-reindex

# Custom data directory
local-index index "$OBSIDIAN_VAULT" --data-dir ~/.local/share/local-index
```

## What to Expect

Progress output goes to **stderr**; the binary exits 0 on success. A successful run
will print lines indicating files discovered, chunks embedded, and total time. There
is no JSON output for the index command — just exit code and stderr logs.

## Error Cases

| Error | Cause | Claude's response |
|-------|-------|-------------------|
| "VOYAGE_API_KEY not set" | API key missing | Tell the user to set `export VOYAGE_API_KEY="..."` and retry |
| No PATH argument / usage error | Vault path missing | Provide path per decision tree above |
| Network error during embedding | Voyage AI API unreachable | Exponential backoff is automatic; if repeated failures, check API key and connectivity |
| "model mismatch" warning | Existing index used a different embedding model | Rerun with `--force-reindex` to rebuild all embeddings from scratch |
