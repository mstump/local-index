# Phase 8 — Technical Research

**Phase:** Search UX Enhancements (WEB-07, WEB-08)  
**Date:** 2026-04-14

## Summary

Implement rerank UX via GET `rerank` boolean aligned with a visible checkbox (default on when `AppState::anthropic_reranker` is `Some`), and server-side snippet highlighting with HTML safety. Use **`askama::Html<String>`** (or equivalent `Markup` in 0.15) for the snippet body so Askama does not double-escape trusted markup built from escaped literals plus literal `<mark>` tags.

## Askama 0.15 — trusted HTML field

- Add a field such as `chunk_html: askama::Html<String>` on `SearchResultView`.
- In `search.html`, replace `{{ result.chunk_text }}` with `{{ result.chunk_html }}` inside `.result-body`.
- Construct `Html(String)` only after: (1) computing highlights on the **plain** preview string, (2) escaping all user-controlled text segments, (3) wrapping match substrings in `<mark>...</mark>`.

## Highlighting algorithm (WEB-08)

1. Split `query` on ASCII whitespace into non-empty terms.
2. For each term, `regex::escape` before inserting into a combined regex: `(?i)(?-u:\b)(term1)(?-u:\b)|...` — use word boundaries appropriate for ASCII-ish note content; document limitation for pure-unicode words.
3. Find non-overlapping matches in the preview `&str` (leftmost-first).
4. Walk the string: between matches, append `html_escape` of slice; for each match append `<mark>` + `html_escape(match)` + `</mark>`.
5. Wrap final string with `Html::new` / constructor per askama 0.15 API.

**Dependencies:** add `regex` (workspace already uses serde-heavy stack; regex is standard). Add `html-escape` **or** inline minimal entity encoding for `&`, `<`, `>`, `"` on segments only — prefer a small crate to avoid bugs.

## Query params (WEB-07)

- Extend `SearchParams` with `rerank: Option<bool>` (serde accepts `true`/`1`/`false`/`0` as needed — document chosen mapping).
- Remove reliance on `no_rerank` for **dashboard** form; keep parsing `no_rerank` optionally for backward compatibility in the same struct if trivial (CONTEXT allows).
- Logic: `let rerank_requested = ... from checkbox param ...;` then `let rerank = state.anthropic_reranker.is_some() && rerank_requested`.
- Template: pass `rerank_available: bool`, `rerank_checked: bool`, `rerank_applied: bool` (for summary badge — true when reranker ran and user had rerank on).

## Tests

- **Unit tests** for highlight helper: multi-term, case-insensitivity, word boundary (e.g. `bar` does not match `foobar`), HTML injection attempt in query (`<script>`) yields escaped text inside `<mark>` or no raw tag injection.
- **Integration:** extend `tests/web_dashboard.rs` or add `tests/search_highlight.rs` with axum `tower::ServiceExt::oneshot` if pattern exists; otherwise document manual verify for checkbox + badge.

## Risks

- Double-escaping if template still auto-escapes `Html` — verify against Askama 0.15 behavior in `read_first`.
- Logging must not add new secret fields (Phase 7 logging stays query/mode/count/latency only).

---

## Validation Architecture

Nyquist validation for this phase is satisfied by:

- **Fast feedback:** `cargo test` after each task touching logic; full `cargo test` after plan complete.
- **Dimension coverage:** WEB-07 covered by handler/template tests or integration HTTP tests; WEB-08 covered by unit tests on pure highlight function plus one XSS-style assertion.
- **Wave 0:** Not required — existing Rust test harness is present (`tests/`, `cargo test`).

---

## RESEARCH COMPLETE
