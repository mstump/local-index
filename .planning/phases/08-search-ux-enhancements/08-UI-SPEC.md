---
phase: 8
slug: search-ux-enhancements
status: draft
shadcn_initialized: false
preset: none
created: 2026-04-14
---

# Phase 8 — UI Design Contract

> Server-rendered dashboard (Askama + `base.html` CSS). No component framework; extend existing tokens and utilities.

---

## Design System

| Property | Value |
|----------|-------|
| Tool | none (vanilla CSS in `templates/base.html`) |
| Preset | local-index dashboard |
| Component library | none |
| Icon library | none (Unicode arrow in result path only) |
| Font | system UI stack (`-apple-system`, Segoe UI, Roboto, …) |

---

## Spacing Scale

Aligned with existing utilities in `base.html` (multiples of 4px):

| Token | Value | Usage |
|-------|-------|-------|
| xs | 4px | `mt-xs`, tight inline gaps |
| sm | 8px | `mt-sm`, compact form row |
| md | 16px | Default vertical rhythm, form padding |
| lg | 24px | Section gaps |
| xl | 32px | Major breaks |

Exceptions: Search form row uses flex wrap; checkbox aligns with input row per CONTEXT D-05.

---

## Typography

| Role | Size | Weight | Line Height |
|------|------|--------|-------------|
| Body | 16px | 400 | 1.5 |
| Label | 14px | 600 | 1.4 (`.label`) |
| h1 (page) | 28px | 600 | 1.2 |
| h2 | 20px | 600 | 1.2 |
| Mono / code | 14px | 400 | 1.5 |

---

## Color

| Role | Value | Usage |
|------|-------|-------|
| Dominant (60%) | `#ffffff` (`--bg`) | Page background |
| Secondary (30%) | `#f5f5f5` (`--bg-secondary`), `#5f6368` (`--text-secondary`) | Nav bar, muted copy, **“(reranked)”** summary suffix (D-04) |
| Accent (10%) | `#1a73e8` (`--accent`) | Links, primary button, focus — **not** used for rerank indicator |
| Highlight (new) | soft amber background on `<mark>` (D-09); text inherits `--text-primary` | Snippet term matches only inside `.result-body` |

Accent reserved for: primary CTA (“Search Notes”), nav active state, `/settings` helper link when rerank disabled.

---

## Layout & Components (Phase 8)

### Search form row (WEB-07)

- **Order:** `[query input] [mode select] [Rerank results checkbox + label] [submit]` on one flex row; wrap on narrow viewports (D-05).
- **Checkbox:** Native `<input type="checkbox" name="rerank" value="1">` (or equivalent GET param contract in PLAN). Visually aligned with baseline of text input.
- **Default:** When `anthropic_reranker` is present, checkbox **checked** by default (CONTEXT D-02).
- **Unavailable:** Checkbox `disabled`, `title` tooltip (short), plus visible link “Settings” → `/settings` (D-06).

### Results summary (WEB-07)

- After successful search with reranking applied: append **literal** ` (reranked)` to the existing “Showing N results…” line — **muted** via `.text-secondary` or equivalent; **once** per page, not per card (D-03, D-04).

### Result snippet (WEB-08)

- **Container:** `.result-body` receives **pre-escaped HTML** from server: plain text escaped, then query terms wrapped in `<mark>...</mark>` (word-boundary, case-insensitive, per term for multi-word `q`).
- **Do not** highlight in file path or heading breadcrumb (D-07).
- **Security:** User query never injected as raw HTML; only escaped literals and controlled `<mark>` tags (roadmap success criterion 3).

### `<mark>` styling

- Background: soft yellow/amber; contrast ≥ informal AA on white; no underline; padding minimal (`0 0.1em` optional).

---

## Copywriting Contract

| Element | Copy |
|---------|------|
| Checkbox label | `Rerank results` |
| Rerank suffix (summary) | ` (reranked)` — lowercase “reranked” inside parens |
| Disabled tooltip | Short explanation: Anthropic API key not configured; point user to Settings (exact string: planner/impl discretion, ≤120 chars) |
| Settings link (disabled rerank) | `Settings` or `Open settings` linking to `/settings` |

---

## Interaction & URL

- Submitting with checkbox checked sends **`rerank=true`** (or `1`) to backend; unchecked sends explicit off or omits param per single consistent pattern (CONTEXT D-01).
- **Do not** show `no_rerank` in the form (legacy may remain server-side only).

---

## Registry Safety

| Registry | Blocks Used | Safety Gate |
|----------|-------------|-------------|
| shadcn official | none | not required |
| Third-party UI | none | not applicable |

---

## Checker Sign-Off

- [ ] Dimension 1 Copywriting: PASS
- [ ] Dimension 2 Visuals: PASS
- [ ] Dimension 3 Color: PASS
- [ ] Dimension 4 Typography: PASS
- [ ] Dimension 5 Spacing: PASS
- [ ] Dimension 6 Registry Safety: PASS

**Approval:** pending — run `/gsd-ui-phase 8` checker pass if workflow requires formal sign-off
