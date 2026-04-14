use std::fmt::Write;

use super::types::SearchResponse;

/// Serialize a SearchResponse to pretty-printed JSON.
pub fn format_json(response: &SearchResponse) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(response)
}

/// Render a SearchResponse as human-readable snippet blocks.
///
/// Format:
/// ```text
/// Found 2 result(s) for "query" (mode: hybrid)
///
/// [1] notes/file.md -- ## Heading  (score: 0.83)
/// ════════════════════════════════════════
/// ## Heading
/// Chunk text here...
/// ```
pub fn format_pretty(response: &SearchResponse) -> String {
    let mut out = String::new();

    if response.results.is_empty() {
        writeln!(
            out,
            "No results found for \"{}\" (mode: {})",
            response.query, response.mode
        )
        .unwrap();
        return out;
    }

    writeln!(
        out,
        "Found {} result(s) for \"{}\" (mode: {})\n",
        response.total, response.query, response.mode
    )
    .unwrap();

    let separator: String = std::iter::repeat('\u{2550}').take(40).collect();
    let mut display_index: usize = 1;

    for result in &response.results {
        // Context chunks get [ctx] prefix, regular results get [N]
        let prefix = if result.is_context == Some(true) {
            "[ctx]".to_string()
        } else {
            let p = format!("[{}]", display_index);
            display_index += 1;
            p
        };

        writeln!(
            out,
            "{} {} -- {}  (score: {:.2})",
            prefix, result.file_path, result.heading_breadcrumb, result.similarity_score
        )
        .unwrap();
        writeln!(out, "{}", separator).unwrap();

        // Show heading breadcrumb as header, then chunk text
        writeln!(out, "{}", result.heading_breadcrumb).unwrap();

        if result.chunk_text.len() > 200 {
            writeln!(out, "{}", &result.chunk_text[..200]).unwrap();
            writeln!(out, "[truncated]").unwrap();
        } else {
            writeln!(out, "{}", result.chunk_text).unwrap();
        }

        writeln!(out).unwrap();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::{LineRange, SearchResponse, SearchResult};

    fn make_result(
        text: &str,
        path: &str,
        breadcrumb: &str,
        score: f64,
        is_context: Option<bool>,
    ) -> SearchResult {
        SearchResult {
            chunk_text: text.to_string(),
            file_path: path.to_string(),
            heading_breadcrumb: breadcrumb.to_string(),
            similarity_score: score,
            semantic_score: Some(score),
            fts_score: None,
            line_range: LineRange { start: 1, end: 10 },
            frontmatter: serde_json::json!({}),
            is_context,
            context_for_index: if is_context == Some(true) {
                Some(0)
            } else {
                None
            },
        }
    }

    #[test]
    fn test_format_json_wrapped_object() {
        let response = SearchResponse {
            query: "test query".to_string(),
            mode: "hybrid".to_string(),
            total: 1,
            results: vec![make_result(
                "Some text",
                "notes/test.md",
                "# Title",
                0.85,
                None,
            )],
        };

        let json_str = format_json(&response).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert!(value.get("query").is_some(), "missing 'query' key");
        assert!(value.get("mode").is_some(), "missing 'mode' key");
        assert!(value.get("total").is_some(), "missing 'total' key");
        assert!(value.get("results").is_some(), "missing 'results' key");
        assert_eq!(value["query"].as_str().unwrap(), "test query");
        assert_eq!(value["total"].as_u64().unwrap(), 1);
    }

    #[test]
    fn test_format_pretty_snippet_blocks() {
        let response = SearchResponse {
            query: "rust".to_string(),
            mode: "semantic".to_string(),
            total: 2,
            results: vec![
                make_result(
                    "Rust is a systems language",
                    "notes/rust.md",
                    "## Intro",
                    0.83,
                    None,
                ),
                make_result(
                    "Ownership and borrowing",
                    "notes/ownership.md",
                    "## Ownership",
                    0.71,
                    None,
                ),
            ],
        };

        let output = format_pretty(&response);
        assert!(output.contains("[1]"), "missing [1] index");
        assert!(output.contains("[2]"), "missing [2] index");
        assert!(output.contains("\u{2550}"), "missing separator character");
        assert!(output.contains("notes/rust.md"), "missing file path");
        assert!(output.contains("0.83"), "missing score");
        assert!(output.contains("0.71"), "missing second score");
    }

    #[test]
    fn test_format_pretty_truncation() {
        let long_text = "A".repeat(300);
        let response = SearchResponse {
            query: "long".to_string(),
            mode: "fts".to_string(),
            total: 1,
            results: vec![make_result(
                &long_text,
                "notes/long.md",
                "# Long",
                0.5,
                None,
            )],
        };

        let output = format_pretty(&response);
        assert!(
            output.contains("[truncated]"),
            "should truncate text over 200 chars"
        );
        // Should NOT contain the full 300-char text
        assert!(!output.contains(&long_text), "full text should not appear");
    }

    #[test]
    fn test_format_pretty_empty() {
        let response = SearchResponse {
            query: "nothing".to_string(),
            mode: "hybrid".to_string(),
            total: 0,
            results: vec![],
        };

        let output = format_pretty(&response);
        assert!(
            output.contains("No results found"),
            "should show no results message"
        );
        assert!(output.contains("nothing"), "should include the query");
    }

    #[test]
    fn test_format_pretty_context_chunks() {
        let response = SearchResponse {
            query: "ctx test".to_string(),
            mode: "semantic".to_string(),
            total: 2,
            results: vec![
                make_result("Main result", "notes/test.md", "# Main", 0.9, None),
                make_result(
                    "Context chunk",
                    "notes/test.md",
                    "# Adjacent",
                    0.0,
                    Some(true),
                ),
            ],
        };

        let output = format_pretty(&response);
        assert!(
            output.contains("[ctx]"),
            "context chunks should have [ctx] prefix"
        );
        assert!(
            output.contains("[1]"),
            "regular results should have numbered prefix"
        );
    }
}
