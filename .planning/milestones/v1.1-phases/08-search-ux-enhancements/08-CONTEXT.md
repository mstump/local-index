# Phase 8: Search UX Enhancements - Context

**Gathered:** 2026-04-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 8 delivers **web-only** UX for (1) explicit reranking control and feedback and (2) safe, visible query-term highlighting in result snippets, as scoped in **WEB-07** and **WEB-08**. No new search modes, CLI changes, or auth.

**Note on defaults:** Roadmap success criteria describe checkbox behavior when checked; the product owner chose **reranking on by default** when a reranker is configured (checkbox starts checked; user may turn off per search).

</domain>

<decisions>
## Implementation Decisions

### Rerank checkbox, URL param, and indicator

- **D-01:** Query string uses **`rerank=true` (or `rerank=1`) only when reranking is requested**; when off, omit `rerank` or send explicit `rerank=false` — planner to pick one consistent pattern. **Do not surface `no_rerank` in the dashboard UI** (legacy query params may still be handled internally if trivial, but the form and docs target `rerank` only).
- **D-02:** When an Anthropic reranker is **available**, the **“Rerank results” checkbox defaults to checked** (rerank on by default). User can uncheck to search without reranking for that request.
- **D-03:** **“(reranked)”** appears **once** in the **results summary** line (same area as “Showing N results…”), not on every card.
- **D-04:** Indicator styling: **plain text in parentheses**, **muted secondary color** (reuse existing secondary text utility / CSS variables).
- **D-05:** Checkbox is on the **same row** as query input, mode `<select>`, and submit (layout may wrap on narrow viewports).

### When reranking is unavailable

- **D-06:** If `ANTHROPIC_API_KEY` is not configured (no reranker): checkbox **disabled**, native **`title` tooltip** with short explanation, plus a **visible link to `/settings`** so users can see credential/source context (not the secret value).

### Query-term highlighting (WEB-08)

- **D-07:** Apply **`<mark>` only inside the result snippet / `result-body`**, not path or breadcrumb — matches requirement language on **chunk_text** and avoids noisy headers.
- **D-08:** Highlighting is **server-side in Rust**: HTML-escape snippet text first, then wrap matches in `<mark>` using **case-insensitive, word-boundary-aware** rules; **multi-word queries** highlight **each term independently**. Pass safe HTML to templates per existing askama patterns (avoid double-escaping `mark` tags).
- **D-09:** **`<mark>` visual:** **soft yellow / amber background**, inherited text color, sufficient contrast on white background.

### Claude's Discretion

- Exact tooltip string for disabled checkbox.
- Whether to accept legacy `no_rerank` in the handler for old bookmarks (not advertised in UI).
- Regex/word-boundary details for edge cases (unicode, hyphenation) within WEB-08 constraints.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements and roadmap

- `.planning/ROADMAP.md` — Phase 8 goal, success criteria, WEB-07/WEB-08 scope
- `.planning/REQUIREMENTS.md` — **WEB-07**, **WEB-08** acceptance rows and traceability table

### Implementation touchpoints

- `src/web/handlers.rs` — `SearchParams`, `search_handler`, `SearchTemplate` / `SearchResultView`
- `templates/search.html` — search form and result cards
- `templates/base.html` — global CSS (`<mark>` styles to add)
- `src/web/context.rs` — `AppState` / reranker availability for template flags
- `src/claude_rerank.rs` — reranking behavior (reference only; no phase change required unless planner finds a gap)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **Askama templates:** `search.html` extends `base.html`; add checkbox + conditional summary text; snippet currently `{{ result.chunk_text }}` — will need a field for **pre-rendered safe HTML** or a dedicated escaped+marked string to avoid XSS.
- **CSS variables:** `:root` in `base.html` defines `--text-secondary`, accents — extend with a highlight token for `<mark>` if useful.

### Established Patterns

- **GET form** drives search; state is reflected in URL query params (`q`, `mode`).
- **Rerank today:** Handler sets `SearchOptions.rerank` from `state.anthropic_reranker.is_some() && !params.no_rerank` — Phase 8 replaces UX with explicit **`rerank`** request param and checkbox, plus default-on when reranker exists.

### Integration Points

- **`/settings`** — already lists credential source; link target for disabled-rerank helper (D-06).
- **Logging:** Phase 7 `web search completed` spans — ensure any new params do not log secrets (still only query/mode/count/latency).

</code_context>

<specifics>
## Specific Ideas

- Owner prefers **rerank on by default** when the reranker is configured (explicitly stated at end of discuss-phase).

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 08-search-ux-enhancements*
*Context gathered: 2026-04-14*
