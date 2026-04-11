---
phase: 05-web-dashboard
created: 2026-04-11
status: complete
---

# Phase 05: Web Dashboard — Discussion Context

## Domain

Add a browser-accessible HTML dashboard to the axum HTTP server already running
in the `daemon` and `serve` commands. Dashboard routes extend the existing
`metrics_router` in `src/daemon/http.rs`.

## Canonical Refs

- `src/daemon/http.rs` — existing axum router; dashboard routes extend this
- `src/cli.rs` — `Serve` command already defined with `--bind`; `Daemon` already has HTTP
- `.planning/REQUIREMENTS.md` — WEB-01 through WEB-06, CLI-05

## Prior Decisions Carrying Forward

- **axum** is already the web framework (in use in Phase 04) — no change
- **askama** for compile-time HTML templates (CLAUDE.md specification)
- **Rust only** — no Node/Python helpers (CLAUDE.md constraint)
- **Default port 3000**, `--bind` flag to override — already in CLI

## Decisions

### Search interactivity
**Pure HTML form — zero JavaScript.**

The search UI uses a standard HTML `<form method="GET" action="/search">` with a
text input, a mode selector (hybrid / semantic / fts), and a submit button. Each
search triggers a full-page reload. No JavaScript ships with the dashboard.

Rationale: local tool, sub-50ms round-trips, no build tooling needed. HTMX or
vanilla fetch would add complexity without meaningful UX benefit for an operator
tool.

### Navigation structure
**Separate routes — one axum handler per view.**

| Route | View |
|-------|------|
| `GET /` | Search UI (default, accepts `?q=&mode=`) |
| `GET /search` | Alias for `/` with query params |
| `GET /index` | File list: per-file chunk count + last-indexed timestamp |
| `GET /status` | Queue depth, last-index time, total chunks/files |
| `GET /settings` | Read-only: config values, credential source, active flags |

A persistent nav bar links between views. Browser back/forward works naturally.
No JS required to switch views.

### `serve` command wiring
**`--data-dir` flag on the `serve` command.**

```
local-index serve \
  --data-dir ~/.local-index \
  --bind 127.0.0.1:3000
```

- `--data-dir` accepts a path, defaults to `$LOCAL_INDEX_DATA_DIR` env var or
  `~/.local-index` if neither is set (same default as `daemon`)
- `serve` opens a `ChunkStore` in read-only mode at the specified path
- If no LanceDB data exists at `--data-dir`, **initialize a new empty DB** (same
  behavior as `index` command on first run) and serve the dashboard showing empty
  state with the message: "No documents indexed yet — run `local-index index
  <path>` to get started."

### Status view freshness
**Manual reload only.**

The `/status` view is static HTML — no meta-refresh, no JS polling. The operator
reloads the page to see current state. Rationale: status is a point-in-time check,
not a live monitor; auto-refresh would add complexity for no practical benefit.

## Out of Scope (Deferred)

- Auto-refresh / live status updates (WebSocket or SSE)
- Dark/light mode toggle
- Authentication or access control
- Mobile-optimized layout
- Search result pagination (v1: just use `--limit`)
