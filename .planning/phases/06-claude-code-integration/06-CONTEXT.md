---
phase: 06-claude-code-integration
created: 2026-04-13
status: complete
---

# Phase 06: Claude Code Integration — Discussion Context

## Domain

Ship skill files and shell wrapper scripts that let Claude Code invoke
`local-index search`, `reindex`, and `status` without human intervention.
All files live in the repo (`.claude/skills/` and `scripts/`).

## Canonical Refs

- `.planning/REQUIREMENTS.md` — INTG-01 through INTG-04
- `src/cli.rs` — authoritative CLI interface: flags, argument names, output format
- `src/search/` — search engine that backs `local-index search`

## Prior Decisions Carrying Forward

- **JSON output format** for `search` (Phase 1/3 decision) — skills can parse this directly
- **VOYAGE_API_KEY** env var required for any search/index operation
- **Rust only** — no Node/Python helpers; wrappers are shell scripts only
- **`index` requires a PATH argument** — no stored-path concept in the binary

## Decisions

### Reindex path strategy
**Both: `$OBSIDIAN_VAULT` env var with fallback to asking the user.**

The `reindex` skill instructs Claude Code to:
1. Check if `$OBSIDIAN_VAULT` (or `$LOCAL_INDEX_VAULT`) is set in the environment
2. If set, use it directly: `local-index index $OBSIDIAN_VAULT`
3. If not set, ask the user: "What path should I reindex?"

Rationale: users who run this regularly will set the env var once; new users or
one-off reindexes can provide the path interactively.

### Skill file depth
**Rich — full context including example output, field interpretation, and follow-up patterns.**

Each skill file includes:
- Invocation command with all relevant flags
- Annotated example JSON output (showing `chunk_text`, `file_path`, `heading_breadcrumb`,
  `similarity_score`, `line_range`, `frontmatter` fields with descriptions)
- Guidance on interpreting scores (cosine similarity, hybrid RRF scores)
- Common follow-up patterns (e.g., search → read file at line range → synthesize)
- Error cases (VOYAGE_API_KEY missing, no index at data-dir, empty results)

Rationale: Claude Code needs rich context to use results intelligently without
re-asking the user about output format or what fields mean.

### Shell wrapper scope
**Thin pass-through scripts.**

Each wrapper is a one-liner that calls `local-index` with the appropriate subcommand
and passes through all arguments. Env var setup (`VOYAGE_API_KEY`, `OBSIDIAN_VAULT`)
is the user's responsibility via `.env` or shell profile.

Example `scripts/search.sh`:
```sh
#!/usr/bin/env sh
exec local-index search "$@"
```

Rationale: wrappers are convenience shims, not setup helpers. The skill files
handle env var context for Claude; humans set up their own environment.

### Binary location
**Assume `local-index` is in PATH.**

All wrappers and skill files assume the binary was installed via `cargo install`
and is available in PATH. Skill files note this assumption explicitly.

No binary discovery logic — no repo-relative fallback, no `LOCAL_INDEX_BIN` env var.

## Out of Scope (Deferred)

- Auto-discovery of vault path (reading from Obsidian config or git root)
- Shell completion scripts (`local-index --completion`)
- Homebrew formula or other package manager distribution
- Claude Code MCP server integration (v2 idea)
