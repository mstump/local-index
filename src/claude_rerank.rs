//! Second-stage search reranking via the Anthropic Messages API.
//!
//! Retrieval (vector / FTS / hybrid) produces a candidate list; this module
//! asks Claude to reorder candidates by relevance to the user query.

use crate::error::LocalIndexError;
use crate::search::types::SearchResult;
use serde::Deserialize;
use serde_json::Value;

const DEFAULT_MODEL: &str = "claude-3-5-haiku-20241022";
const MAX_EXCERPT_CHARS: usize = 800;
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Client for Claude-based search reranking.
#[derive(Clone, Debug)]
pub struct AnthropicReranker {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RerankPayload {
    indices: Vec<usize>,
}

impl AnthropicReranker {
    /// Build from `ANTHROPIC_API_KEY` and optional `LOCAL_INDEX_RERANK_MODEL`.
    pub fn try_from_env() -> Option<Self> {
        let key = std::env::var("ANTHROPIC_API_KEY").ok()?;
        if key.is_empty() {
            return None;
        }
        Some(Self::new(key))
    }

    pub fn new(api_key: String) -> Self {
        let model = std::env::var("LOCAL_INDEX_RERANK_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url: "https://api.anthropic.com".to_string(),
        }
    }

    /// For tests: redirect API calls to a wiremock server.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Rerank `candidates` for `query`; returns the same items reordered with
    /// updated `similarity_score` (rank-based, higher = more relevant).
    pub async fn rerank(
        &self,
        query: &str,
        candidates: Vec<SearchResult>,
    ) -> Result<Vec<SearchResult>, LocalIndexError> {
        if candidates.len() <= 1 {
            return Ok(candidates);
        }

        let prompt = build_user_prompt(query, &candidates);
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": [{
                "role": "user",
                "content": prompt
            }]
        });

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LocalIndexError::Rerank(format!("rerank HTTP error: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(LocalIndexError::Rerank(format!(
                "Anthropic rerank API error {}: {}",
                status, text
            )));
        }

        let parsed: MessagesResponse = resp.json().await.map_err(|e| {
            LocalIndexError::Rerank(format!("rerank response JSON error: {}", e))
        })?;

        let text = parsed
            .content
            .iter()
            .find(|b| b.block_type == "text")
            .and_then(|b| b.text.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("");

        let order = parse_rerank_json_content(text, candidates.len()).map_err(LocalIndexError::Rerank)?;
        Ok(apply_rerank_order(candidates, &order))
    }
}

fn build_user_prompt(query: &str, candidates: &[SearchResult]) -> String {
    let mut lines = String::new();
    lines.push_str(
        "You are ranking search results for a personal markdown notes index.\n\
         Given the user query and numbered candidates, reply with ONLY a JSON object of the form:\n\
         {\"indices\":[...]}\n\
         where \"indices\" is a permutation of candidate indices 0..n-1 (integers), \
         ordered from most relevant to least relevant.\n\
         Do not include markdown fences or any other text.\n\n",
    );
    lines.push_str("User query:\n");
    lines.push_str(query);
    lines.push_str("\n\nCandidates:\n");
    for (i, c) in candidates.iter().enumerate() {
        let excerpt = truncate_chars(&c.chunk_text, MAX_EXCERPT_CHARS);
        lines.push_str(&format!(
            "\n--- candidate {} ---\nfile: {}\nheading: {}\nlines: {}-{}\nexcerpt:\n{}\n",
            i,
            c.file_path,
            c.heading_breadcrumb,
            c.line_range.start,
            c.line_range.end,
            excerpt
        ));
    }
    lines
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// Extract `indices` from model output; tolerates a single ```json fenced block.
pub fn parse_rerank_json_content(text: &str, num_candidates: usize) -> Result<Vec<usize>, String> {
    let trimmed = text.trim();
    let json_str = extract_json_object_str(trimmed)?;
    let payload: RerankPayload = serde_json::from_str(json_str)
        .map_err(|e| format!("invalid rerank JSON: {}", e))?;
    validate_order(&payload.indices, num_candidates)
}

fn extract_json_object_str(text: &str) -> Result<&str, String> {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        if v.get("indices").is_some() {
            return Ok(text);
        }
    }
    // Strip ```json ... ``` wrapper if present
    let start = text.find('{').ok_or_else(|| "no JSON object in rerank response".to_string())?;
    let end = text.rfind('}').ok_or_else(|| "no closing brace in rerank response".to_string())?;
    if end < start {
        return Err("malformed JSON span in rerank response".to_string());
    }
    Ok(&text[start..=end])
}

fn validate_order(indices: &[usize], n: usize) -> Result<Vec<usize>, String> {
    if n == 0 {
        return Ok(vec![]);
    }
    if indices.len() != n {
        return Err(format!(
            "expected {} indices in rerank output, got {}",
            n,
            indices.len()
        ));
    }
    let mut seen = vec![false; n];
    for &i in indices {
        if i >= n {
            return Err(format!("index {} out of range for n={}", i, n));
        }
        if seen[i] {
            return Err(format!("duplicate index {} in rerank output", i));
        }
        seen[i] = true;
    }
    Ok(indices.to_vec())
}

/// Reorder `candidates` by `order` (permutation of 0..n-1). Missing indices are
/// appended in stable original order. Updates `similarity_score` from rank.
pub fn apply_rerank_order(candidates: Vec<SearchResult>, order: &[usize]) -> Vec<SearchResult> {
    let n = candidates.len();
    if n == 0 {
        return candidates;
    }
    let mut taken = vec![false; n];
    let mut out: Vec<SearchResult> = Vec::with_capacity(n);
    for &i in order {
        if i < n && !taken[i] {
            taken[i] = true;
            out.push(candidates[i].clone());
        }
    }
    for i in 0..n {
        if !taken[i] {
            out.push(candidates[i].clone());
        }
    }
    let count = out.len().max(1);
    let denom = (count - 1).max(1) as f64;
    for (rank, r) in out.iter_mut().enumerate() {
        r.similarity_score = 1.0 - (rank as f64 / denom);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::LineRange;

    #[test]
    fn parse_plain_json() {
        let order = parse_rerank_json_content(r#"{"indices":[2,0,1]}"#, 3).unwrap();
        assert_eq!(order, vec![2, 0, 1]);
    }

    #[test]
    fn parse_fenced_json() {
        let text = "```json\n{\"indices\":[1,0]}\n```";
        let order = parse_rerank_json_content(text, 2).unwrap();
        assert_eq!(order, vec![1, 0]);
    }

    #[test]
    fn parse_rejects_wrong_len() {
        let err = parse_rerank_json_content(r#"{"indices":[0]}"#, 2).unwrap_err();
        assert!(err.contains("expected 2 indices"));
    }

    #[test]
    fn parse_rejects_duplicate() {
        let err = parse_rerank_json_content(r#"{"indices":[0,0]}"#, 2).unwrap_err();
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn parse_rejects_out_of_range() {
        let err = parse_rerank_json_content(r#"{"indices":[0,2]}"#, 2).unwrap_err();
        assert!(err.contains("out of range"));
    }

    #[test]
    fn apply_order_updates_scores() {
        let mut a = dummy_result("a");
        a.similarity_score = 0.1;
        let mut b = dummy_result("b");
        b.similarity_score = 0.9;
        let out = apply_rerank_order(vec![a, b], &[1, 0]);
        assert_eq!(out[0].chunk_text, "b");
        assert_eq!(out[1].chunk_text, "a");
        assert!((out[0].similarity_score - 1.0).abs() < 1e-9);
        assert!((out[1].similarity_score - 0.0).abs() < 1e-9);
    }

    #[test]
    fn apply_order_appends_missing_indices() {
        let a = dummy_result("a");
        let b = dummy_result("b");
        let c = dummy_result("c");
        let out = apply_rerank_order(vec![a, b, c], &[2]);
        assert_eq!(out[0].chunk_text, "c");
        assert!(out.iter().any(|r| r.chunk_text == "a"));
        assert!(out.iter().any(|r| r.chunk_text == "b"));
        assert_eq!(out.len(), 3);
    }

    fn dummy_result(label: &str) -> SearchResult {
        SearchResult {
            chunk_text: label.to_string(),
            file_path: "f.md".to_string(),
            heading_breadcrumb: "# H".to_string(),
            similarity_score: 0.5,
            semantic_score: None,
            fts_score: None,
            line_range: LineRange { start: 1, end: 2 },
            frontmatter: serde_json::json!({}),
            is_context: None,
            context_for_index: None,
        }
    }
}
