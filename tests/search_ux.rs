//! Phase 8 search UX: safe HTML highlighting for web snippets.

use local_index::web::highlight_query_terms;

#[test]
fn highlights_multiple_terms_case_insensitive() {
    let out = highlight_query_terms("Foo is bar", "foo bar");
    assert_eq!(out, "<mark>Foo</mark> is <mark>bar</mark>");
}

#[test]
fn word_boundary_bar_not_inside_foobar() {
    let out = highlight_query_terms("foobar bar", "bar");
    // Only the standalone "bar" should be marked, not substring inside "foobar"
    assert_eq!(out, "foobar <mark>bar</mark>");
}

#[test]
fn xss_angle_brackets_and_marks_are_safe() {
    // Match literal "x" inside text that contains angle brackets — markup must stay escaped.
    let out = highlight_query_terms("<em>x</em>", "x");
    assert!(
        !out.contains("<em>"),
        "raw tags must not appear in output: {out}"
    );
    assert!(out.contains("&lt;"), "expected escaped lt: {out}");
    assert!(out.contains("<mark>"), "expected highlight wrapper: {out}");
}

#[test]
fn empty_query_escapes_preview_only() {
    let out = highlight_query_terms("a < b", "   ");
    assert!(!out.contains("<mark>"));
    assert!(out.contains("&lt;"));
}
