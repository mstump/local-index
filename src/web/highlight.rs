//! Case-insensitive, word-boundary-aware query highlighting for HTML snippets.

use std::borrow::Cow;

use regex::Regex;

/// Wraps each query term match in `<mark>...</mark>` with HTML entity encoding on all text.
/// Empty or whitespace-only `query` returns a fully escaped preview with no `<mark>` tags.
pub fn highlight_query_terms(preview: &str, query: &str) -> String {
    let terms: Vec<&str> = query
        .split(|c: char| c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .collect();

    if terms.is_empty() {
        return html_escape::encode_text(preview).into_owned();
    }

    let mut pattern = String::from("(?i)(?:");
    for (i, term) in terms.iter().enumerate() {
        if i > 0 {
            pattern.push('|');
        }
        pattern.push_str(r"\b(?:");
        pattern.push_str(&regex::escape(term));
        pattern.push_str(r")\b");
    }
    pattern.push(')');

    let Ok(re) = Regex::new(&pattern) else {
        return html_escape::encode_text(preview).into_owned();
    };

    let mut out =
        String::with_capacity(preview.len().saturating_add(terms.len().saturating_mul(14)));
    let mut last = 0usize;
    for m in re.find_iter(preview) {
        push_encoded(&mut out, &preview[last..m.start()]);
        out.push_str("<mark>");
        push_encoded(&mut out, m.as_str());
        out.push_str("</mark>");
        last = m.end();
    }
    push_encoded(&mut out, &preview[last..]);
    out
}

fn push_encoded(out: &mut String, s: &str) {
    match html_escape::encode_text(s) {
        Cow::Borrowed(b) => out.push_str(b),
        Cow::Owned(o) => out.push_str(&o),
    }
}
