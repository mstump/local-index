# Skill: local-index search

## Purpose

This skill enables Claude to perform semantic, full-text, or hybrid search over a locally
indexed markdown vault and interpret the results without human guidance. Claude can invoke
the `local-index search` command directly via the Bash tool and understand all returned
fields without asking the user for format guidance.

## Prerequisites

- `local-index` binary installed (`cargo install local-index`) and available in PATH
- `VOYAGE_API_KEY` environment variable set (required for semantic and hybrid modes)
- Vault indexed at least once via `local-index index <path>`
- Optional: `LOCAL_INDEX_DATA_DIR` if using a non-default data directory
- Optional: `ANTHROPIC_API_KEY` enables result reranking (Claude); omit for retrieval-only scores

## Invocation

```sh
local-index search "<QUERY>" [--limit N] [--min-score F] [--mode semantic|fts|hybrid] \
  [--path-filter PATH_PREFIX] [--tag-filter TAG] [--context N] [--format json] [--no-rerank]
```

`--format json` is the default and is required for machine-readable output.

## All Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--limit N` / `-n N` | 10 | Maximum number of results to return |
| `--min-score F` | (none) | Minimum score threshold 0.0–1.0; omit to return top N regardless of score |
| `--mode` | hybrid | `hybrid` — fuses BM25+vector via RRF (best overall); `semantic` — vector ANN only; `fts` — full-text BM25 only |
| `--path-filter STR` | (none) | Restrict results to files whose path starts with this prefix |
| `--tag-filter STR` | (none) | Restrict results to files with this frontmatter tag |
| `--context N` | 0 | Include N surrounding chunks before/after each match for richer context |
| `--no-rerank` | off | Skip reranking even when `ANTHROPIC_API_KEY` is set (retrieval order/scores only) |
| `--data-dir PATH` | platform default | Override index data directory (env: `LOCAL_INDEX_DATA_DIR`) |

## Example Invocation

```sh
local-index search "how to configure git rebase" --limit 5 --mode hybrid
```

## Annotated Example JSON Output

```json
[
  {
    "chunk_text": "To configure interactive rebase, set core.editor in your git config...",
                                   // The actual text content of this chunk
    "file_path": "git/rebase.md",  // Path relative to the vault root
    "heading_breadcrumb": "## Git Workflow > ### Rebase Strategy",
                                   // Full heading hierarchy from the document
    "similarity_score": 0.87,      // 0.0–1.0; hybrid mode uses RRF score (higher = better match)
    "line_range": { "start": 42, "end": 67 },
                                   // Line numbers in the source file for this chunk
    "frontmatter": {               // YAML frontmatter parsed from the source file
      "tags": ["git", "workflow"],
      "aliases": [],
      "date": "2024-03-15"
    }
  },
  {
    "chunk_text": "Rebase vs merge: prefer rebase for local cleanup, merge for shared branches...",
    "file_path": "git/branching-strategy.md",
    "heading_breadcrumb": "## Branching > ### When to Rebase",
    "similarity_score": 0.71,
    "line_range": { "start": 12, "end": 28 },
    "frontmatter": {
      "tags": ["git"],
      "aliases": ["merge-vs-rebase"],
      "date": "2024-01-08"
    }
  }
]
```

## Score Interpretation

- **Hybrid RRF scores** (no rerank): 0.01–0.10 is the typical range; scores above 0.05 are strong matches
- **After rerank** (`ANTHROPIC_API_KEY` set): `similarity_score` is rank-based (1.0 = top result)
- **Semantic cosine scores**: 0.0–1.0; above 0.75 is a strong semantic match
- **FTS scores**: rank-based; higher means more keyword overlap
- Empty array `[]` means no results above the threshold — try lowering `--min-score` or broadening the query

## Common Follow-Up Patterns

1. **Read the source for more context**: Use the Read tool on the returned `file_path` at lines
   `line_range.start` through `line_range.end` to see the full passage around the chunk.
2. **Navigate by heading**: Use `heading_breadcrumb` to understand the document structure and
   locate the relevant section in the file.
3. **Narrow by directory**: Run a follow-up search with `--path-filter <dir>` to find related
   notes in the same directory as a high-scoring result.
4. **Filter by topic tag**: Use `--tag-filter <tag>` from the `frontmatter.tags` of a result
   to restrict the next search to notes with that topic.
5. **Expand context**: Add `--context 1` or `--context 2` to retrieve surrounding chunks and
   reconstruct a broader passage when the chunk alone is not enough to answer the question.

## Error Cases

| Error | Cause | Claude's response |
|-------|-------|-------------------|
| Exit non-zero + "VOYAGE_API_KEY not set" | API key missing | Tell the user to set `export VOYAGE_API_KEY="..."` and retry |
| Exit non-zero + "no index found" or "table not found" | Vault not indexed yet | Use the reindex skill: `local-index index <vault-path>` |
| Empty JSON array `[]` | No results above threshold | Suggest broadening the query or removing `--min-score`; try `--mode fts` for keyword fallback |
| Exit non-zero + network/connection error | Voyage AI API unreachable | Check internet connectivity and VOYAGE_API_KEY validity |
