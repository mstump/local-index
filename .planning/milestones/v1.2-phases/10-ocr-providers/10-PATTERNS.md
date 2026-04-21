# Phase 10 — Pattern Map

## Analog: embedder / HTTP client

| New / touched | Analog | Notes |
|---------------|--------|--------|
| Optional third-party HTTP + env-based construction | `AnthropicAssetClient::new_from_env()` in `src/pipeline/assets/anthropic_extract.rs` | Same error tone (`LocalIndexError`), `reqwest` + JSON |
| Credential helpers | `resolve_anthropic_key_for_assets` in `src/credentials.rs` | Add parallel `resolve_*` for Google / Document AI settings |
| CLI env + global-style flags | `skip-asset-processing` / `LOCAL_INDEX_SKIP_ASSET_PROCESSING` in `src/cli.rs` | Add `--ocr-provider` + env on **Index** and **Daemon** only |

## Analog: ingest orchestration

| Integration | File | Pattern |
|-------------|------|---------|
| NeedsVision branch | `src/pipeline/assets/ingest.rs` | Replace direct `describe_raster_page` loop with **`OcrService`** (enum) dispatch returning per-page strings |

## Tests

| Pattern | Location |
|---------|----------|
| wiremock Anthropic | `tests/anthropic_assets_mock.rs` |
| Copy for Document AI | New `tests/document_ai_mock.rs` — mock `*:process` response body with `document.text` |

---

## PATTERN MAPPING COMPLETE
