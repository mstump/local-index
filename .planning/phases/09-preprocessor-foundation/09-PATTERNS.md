# Phase 9 — Pattern mapping

**Status:** ## PATTERN MAPPING COMPLETE

## Summary

New code follows existing **pipeline → chunk → embed → store** flow. No new subcommand.

---

## Analog files

| New responsibility | Closest existing analog | Pattern to copy |
|--------------------|-------------------------|-----------------|
| Vault walk + ignore rules | `src/pipeline/walker.rs` (`WalkDir`, hidden skip) | Add `ignore::WalkBuilder` parallel or merge; keep vault-root relative paths |
| CLI global flags + env | `src/cli.rs` (`#[arg(env = "...")]`) | Add `--no-assets` + `LOCAL_INDEX_PROCESS_ASSETS` (default true) — names per planner |
| Daemon file events | `src/daemon/processor.rs` (`reindex_file`, extension checks) | Add asset extensions alongside `.md` |
| Anthropic HTTP JSON | `src/claude_rerank.rs` (version header, `reqwest`, errors) | New module for vision messages; same TLS stack |
| Chunk storage provenance | `src/pipeline/store.rs` (`file_path` column) | Always pass asset `PathBuf` into `Chunk` |
| One-shot index loop | `src/main.rs` (`Index` command) | Interleave or second pass over `discover_asset_files` |
| Credential errors | `src/credentials.rs` (`LocalIndexError::Credential`) | Mirror message style for `ANTHROPIC_API_KEY` when asset path requires Claude |

---

## Code excerpts (reference)

**Walker (markdown only today):**

```6:20:src/pipeline/walker.rs
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
                    if entry.path().extension().is_some_and(|ext| ext == "md") {
```

**Chunk type (file_path = vault-relative):**

```22:28:src/types.rs
pub struct Chunk {
    /// Vault-relative file path
    pub file_path: PathBuf,
    /// Heading hierarchy active at the start of this chunk (e.g., "# H1 > ## H2").
    /// Used for display and filtering. A chunk body may contain text from multiple headings.
    pub heading_breadcrumb: String,
```

**Reranker env key pattern:**

```42:45:src/claude_rerank.rs
    /// Build from `ANTHROPIC_API_KEY` and optional `LOCAL_INDEX_RERANK_MODEL`.
    pub fn try_from_env() -> Option<Self> {
        let key = std::env::var("ANTHROPIC_API_KEY").ok()?;
```

---

## Anti-patterns

- Writing extracted text as `notes/foo.pdf.md` in the vault (violates D-02).
- Using cache file path as `Chunk.file_path` (violates D-01).
- Spawning a separate long-lived “preprocessor” process (violates D-03).
