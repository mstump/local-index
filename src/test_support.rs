//! Test-only fixtures re-exported for integration tests under `tests/`.
//!
//! This module is public so integration tests (which compile as separate
//! crates) can reach hand-crafted fixtures defined inside the library.
//! Nothing in `src/main.rs` imports it; its contents are dead code from
//! production call sites.

pub use crate::pipeline::assets::fixture_single_page_pdf_with_embedded_image;
