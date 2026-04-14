# Phase 8 — Pattern Map

## Analog: search handler + template

| Planned change | Closest analog | Notes |
|----------------|----------------|-------|
| Extend GET search params | `src/web/handlers.rs` `SearchParams`, `search_handler` | Same file as Phase 7 logging |
| Template fields for search | `templates/search.html` + `SearchTemplate` in `handlers.rs` | Mirrors existing `query` / `mode` / `results` |
| AppState capability flags | `src/web/context.rs` `AppState` | `anthropic_reranker: Option<_>` already exists |
| Global CSS tokens | `templates/base.html` `:root` | Add `--mark-bg` or style `mark {}` |

## Data flow

```
GET /search?q=&mode=&rerank=
  → search_handler reads SearchParams + AppState
  → SearchOptions.rerank from reranker present + user intent
  → build SearchResultView with Html snippet per result
  → SearchTemplate → askama render
```

## Code excerpt (current snippet line)

`templates/search.html` line 30: `<div class="result-body">{{ result.chunk_text }}</div>` — replace with trusted HTML field once `SearchResultView` carries `chunk_html: Html<String>`.

---

## PATTERN MAPPING COMPLETE
