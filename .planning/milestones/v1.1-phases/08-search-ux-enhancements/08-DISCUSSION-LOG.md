# Phase 8: Search UX Enhancements - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-14
**Phase:** 8-Search UX Enhancements
**Areas discussed:** Rerank URL/badge/layout, Disabled rerank UX, Highlight scope, Highlight implementation, Mark styling

---

## Rerank URL contract

| Option | Description | Selected |
|--------|-------------|----------|
| Primary `rerank` only | Form uses `rerank=true` when checked; omit or false when off; do not advertise `no_rerank` in UI | ✓ |
| Both for compat | Also accept legacy `no_rerank` from bookmarks | |
| Claude decides | Simplest implementation meeting WEB-07 | |

**User's choice:** Primary `rerank` only (UI contract).
**Notes:** Legacy handler behavior left to planner discretion (see CONTEXT Claude's Discretion).

---

## “(reranked)” placement

| Option | Description | Selected |
|--------|-------------|----------|
| Summary only | Once next to “Showing N results…” | ✓ |
| Per card | On every result | |
| Both | Summary and each card | |
| Claude decides | Layout fit | |

**User's choice:** Summary only.

---

## “(reranked)” styling

| Option | Description | Selected |
|--------|-------------|----------|
| Plain muted | Parentheses, secondary/muted color | ✓ |
| Pill | Badge with border/background | |
| Claude decides | Subtle + accessible | |

**User's choice:** Plain muted.

---

## Checkbox layout

| Option | Description | Selected |
|--------|-------------|----------|
| Same row | With query, mode, submit; wrap on narrow screens | ✓ |
| Second row | Under main controls | |
| Claude decides | Responsive | |

**User's choice:** Same row.

---

## Disabled rerank explanation

| Option | Description | Selected |
|--------|-------------|----------|
| Tooltip only | `title` on disabled checkbox | |
| Tooltip + line | Short static hint, no link | |
| Tooltip + settings link | `title` plus link to `/settings` | ✓ |

**User's choice:** Tooltip plus link to `/settings`.

---

## Highlight scope

| Option | Description | Selected |
|--------|-------------|----------|
| Snippet only | `result-body` / chunk preview only | ✓ |
| Snippet + header | Path and breadcrumb too | |
| Claude decides | REQ-first | |

**User's choice:** Snippet only.

---

## Highlight implementation location

| Option | Description | Selected |
|--------|-------------|----------|
| Server-side Rust | Escape, then inject `<mark>`; pass to askama safely | ✓ |
| Client-side JS | Not preferred | |
| Claude decides | Must satisfy escape-before-mark | |

**User's choice:** Server-side Rust.

---

## `<mark>` CSS

| Option | Description | Selected |
|--------|-------------|----------|
| Soft yellow | Classic highlight on white | ✓ |
| Accent tint | Light blue from `--accent` | |
| Claude decides | WCAG | |

**User's choice:** Soft yellow.

---

## Additional product note

**User's choice (freeform):** Reranking should be **enabled by default** when a reranker is available (checkbox defaults on).

**Notes:** Recorded in CONTEXT as D-02; reconciled with WEB-07 “when checked sends rerank=true” by interpreting success criteria as behavior of the checked state, not mandatory default-off.

---

## Claude's Discretion

- Exact disabled-checkbox tooltip copy.
- Optional legacy `no_rerank` parsing for old URLs.
- Word-boundary edge cases within WEB-08.

## Deferred Ideas

None.
