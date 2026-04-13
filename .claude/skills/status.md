# Skill: local-index status

## Purpose

This skill enables Claude to check the current state of the local-index: total chunks
indexed, files indexed, last index timestamp, embedding model, and queue depth.
Use this to confirm the index is healthy before running a search, or to diagnose
why search results seem stale or missing.

## Prerequisites

- `local-index` binary installed (`cargo install local-index`) and available in PATH
- An existing index (vault indexed at least once via `local-index index <path>`)
- Optional: `LOCAL_INDEX_DATA_DIR` if using a non-default data directory
- **`VOYAGE_API_KEY` is NOT required for the status command**

## Invocation

```sh
local-index status [--data-dir PATH]
```

## Flag

| Flag | Description |
|------|-------------|
| `--data-dir PATH` | Override index data directory (env: `LOCAL_INDEX_DATA_DIR`) |

## Example Output Fields

The status command reports the following fields (exact format may vary by version):

| Field | Description |
|-------|-------------|
| Total chunks indexed | Number of text chunks stored in the vector index |
| Total files indexed | Number of source markdown files processed |
| Last index timestamp | When the most recent full or incremental index completed |
| Embedding model | Name/ID of the model used to embed chunks (e.g. `voyage-3.5`) |
| Pending queue depth | Number of files queued for re-indexing (daemon mode only) |
| Stale file count | Files modified on disk after the last index run |

## Interpreting Output

- **Stale file count > 0**: Files have changed since the last index. Run the reindex skill
  to bring the index up to date.
- **No index found / uninitialized state**: The vault has not been indexed yet. Use the
  reindex skill: `local-index index <vault-path>`.
- **Embedding model mismatch** (compared to what you expect): Run with `--force-reindex`
  to rebuild from scratch with the current model.

## Error Cases

| Error | Cause | Claude's response |
|-------|-------|-------------------|
| No index found | Vault not indexed yet | Suggest running `local-index index <vault-path>` first |
| Binary not found in PATH | `local-index` not installed | Suggest `cargo install local-index` |
