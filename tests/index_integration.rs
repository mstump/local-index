use std::fs;
use std::process::Command;

fn cargo_run_index(vault_path: &str) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--quiet", "--", "--log-level", "warn", "index", vault_path])
        .output()
        .expect("failed to run cargo")
}

#[test]
fn test_index_markdown_vault() {
    let dir = tempfile::tempdir().unwrap();

    // note1.md: frontmatter + two headings
    fs::write(
        dir.path().join("note1.md"),
        "---\ntags:\n  - test\n---\n# Heading One\nBody of heading one.\n## Sub Heading\nBody of sub heading.\n",
    )
    .unwrap();

    // subdir/note2.md: nested file with one heading
    let subdir = dir.path().join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(
        subdir.join("note2.md"),
        "# Single Heading\nSome content here.\n",
    )
    .unwrap();

    // readme.txt: non-markdown, should be skipped
    fs::write(dir.path().join("readme.txt"), "Not a markdown file").unwrap();

    let output = cargo_run_index(dir.path().to_str().unwrap());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "index should exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Indexed 2 files"),
        "should index exactly 2 .md files, got: {}",
        stdout
    );

    // Parse JSON lines (skip the summary line)
    let json_lines: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    assert!(
        !json_lines.is_empty(),
        "should have at least one JSON chunk in output"
    );

    // At least one chunk should have a non-empty heading_breadcrumb
    assert!(
        json_lines
            .iter()
            .any(|v| v["heading_breadcrumb"].as_str().unwrap_or("") != ""),
        "at least one chunk should have a heading breadcrumb"
    );

    // At least one chunk should have frontmatter tags containing "test"
    assert!(
        json_lines.iter().any(|v| {
            v["frontmatter"]["tags"]
                .as_array()
                .map(|tags| tags.iter().any(|t| t.as_str() == Some("test")))
                .unwrap_or(false)
        }),
        "at least one chunk should have frontmatter tag 'test'"
    );
}

#[test]
fn test_index_empty_dir() {
    let dir = tempfile::tempdir().unwrap();

    let output = cargo_run_index(dir.path().to_str().unwrap());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "empty dir index should exit 0");
    assert!(
        stdout.contains("Indexed 0 files"),
        "should report 0 files indexed, got: {}",
        stdout
    );
}

#[test]
fn test_index_nonexistent_path() {
    let output = cargo_run_index("/tmp/nonexistent_dir_xyz_local_index_test");
    assert!(
        !output.status.success(),
        "nonexistent path should exit non-zero"
    );
}

#[test]
fn test_index_frontmatter_preserved() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("meta.md"),
        "---\ntags:\n  - obsidian\n  - notes\ntitle: Test Note\n---\n# Content\nSome body text.\n",
    )
    .unwrap();

    let output = cargo_run_index(dir.path().to_str().unwrap());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());

    let json_lines: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    assert!(!json_lines.is_empty(), "should produce at least one chunk");

    let chunk = &json_lines[0];
    let tags = chunk["frontmatter"]["tags"].as_array().expect("tags should be array");
    assert!(
        tags.iter().any(|t| t.as_str() == Some("obsidian")),
        "should have 'obsidian' tag"
    );
    assert!(
        tags.iter().any(|t| t.as_str() == Some("notes")),
        "should have 'notes' tag"
    );
    assert_eq!(
        chunk["frontmatter"]["title"].as_str(),
        Some("Test Note"),
        "title should be preserved"
    );
}

#[test]
fn test_index_heading_breadcrumbs() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("headings.md"),
        "# Chapter\n## Section\n### Subsection\nContent here\n## Another\nMore content\n",
    )
    .unwrap();

    let output = cargo_run_index(dir.path().to_str().unwrap());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());

    let json_lines: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    // Find chunk with "Content here"
    let subsection_chunk = json_lines
        .iter()
        .find(|v| {
            v["body"]
                .as_str()
                .map(|b| b.contains("Content here"))
                .unwrap_or(false)
        })
        .expect("should have a chunk containing 'Content here'");

    assert_eq!(
        subsection_chunk["heading_breadcrumb"].as_str().unwrap(),
        "# Chapter > ## Section > ### Subsection",
        "subsection breadcrumb should show full hierarchy"
    );

    // Find chunk with "More content"
    let another_chunk = json_lines
        .iter()
        .find(|v| {
            v["body"]
                .as_str()
                .map(|b| b.contains("More content"))
                .unwrap_or(false)
        })
        .expect("should have a chunk containing 'More content'");

    assert_eq!(
        another_chunk["heading_breadcrumb"].as_str().unwrap(),
        "# Chapter > ## Another",
        "sibling heading should reset breadcrumb"
    );
}
