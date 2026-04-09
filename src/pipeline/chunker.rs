use std::path::Path;

use pulldown_cmark::{Event, HeadingLevel, MetadataBlockKind, Options, Parser, Tag, TagEnd};

use crate::error::LocalIndexError;
use crate::types::{Chunk, ChunkedFile, Frontmatter};

/// Parse markdown content into chunks split by heading, with frontmatter extraction.
pub fn chunk_markdown(content: &str, file_path: &Path) -> Result<ChunkedFile, LocalIndexError> {
    let options =
        Options::ENABLE_YAML_STYLE_METADATA_BLOCKS | Options::ENABLE_HEADING_ATTRIBUTES;

    let mut heading_stack: Vec<(HeadingLevel, String)> = Vec::new();
    let mut current_heading_text = String::new();
    let mut in_heading = false;
    let mut in_metadata = false;
    let mut yaml_text = String::new();
    let mut current_body = String::new();
    let mut current_line_start: usize = 1;
    let mut current_heading_level: u8 = 0;
    let mut current_breadcrumb = String::new();
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut frontmatter = Frontmatter::default();
    let mut frontmatter_parsed = false;

    for (event, range) in Parser::new_ext(content, options).into_offset_iter() {
        match event {
            Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                in_metadata = true;
            }
            Event::Text(ref text) if in_metadata => {
                yaml_text.push_str(text);
            }
            Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                in_metadata = false;
                frontmatter_parsed = true;
                match serde_yml::from_str::<Frontmatter>(&yaml_text) {
                    Ok(fm) => frontmatter = fm,
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to parse YAML frontmatter; treating as content");
                        frontmatter = Frontmatter::default();
                    }
                }
                // Update line start to after frontmatter
                current_line_start = byte_offset_to_line(content, range.end);
            }
            Event::Start(Tag::Heading { level, .. }) => {
                // Finalize previous chunk if there's accumulated body
                let trimmed = current_body.trim();
                if !trimmed.is_empty() {
                    let line_end = byte_offset_to_line(content, range.start.saturating_sub(1));
                    chunks.push(Chunk {
                        file_path: file_path.to_path_buf(),
                        heading_breadcrumb: current_breadcrumb.clone(),
                        heading_level: current_heading_level,
                        body: trimmed.to_string(),
                        line_start: current_line_start,
                        line_end,
                        frontmatter: frontmatter.clone(),
                    });
                }
                current_body.clear();

                in_heading = true;
                current_heading_text.clear();

                // Store the level for this heading
                current_heading_level = heading_level_to_u8(level);

                // Update the heading stack
                update_heading_stack(&mut heading_stack, level, String::new());
            }
            Event::Text(ref text) if in_heading => {
                current_heading_text.push_str(text);
            }
            Event::Code(ref code) if in_heading => {
                current_heading_text.push_str(code);
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                // Update the last entry in heading_stack with the collected text
                if let Some(last) = heading_stack.last_mut() {
                    last.1 = current_heading_text.clone();
                }
                current_breadcrumb = breadcrumb(&heading_stack);
                current_line_start = byte_offset_to_line(content, range.end);
            }
            Event::Text(ref text) if !in_heading && !in_metadata => {
                current_body.push_str(text);
            }
            Event::Code(ref code) if !in_heading && !in_metadata => {
                current_body.push_str(code);
            }
            Event::SoftBreak if !in_heading && !in_metadata => {
                current_body.push('\n');
            }
            Event::HardBreak if !in_heading && !in_metadata => {
                current_body.push('\n');
            }
            _ => {}
        }
    }

    // Finalize last chunk
    let trimmed = current_body.trim();
    if !trimmed.is_empty() {
        let line_end = byte_offset_to_line(content, content.len().saturating_sub(1));
        chunks.push(Chunk {
            file_path: file_path.to_path_buf(),
            heading_breadcrumb: current_breadcrumb,
            heading_level: current_heading_level,
            body: trimmed.to_string(),
            line_start: current_line_start,
            line_end,
            frontmatter: frontmatter.clone(),
        });
    }

    // If frontmatter was not parsed (no metadata block), keep default
    let _ = frontmatter_parsed;

    Ok(ChunkedFile {
        file_path: file_path.to_path_buf(),
        frontmatter,
        chunks,
    })
}

fn update_heading_stack(
    stack: &mut Vec<(HeadingLevel, String)>,
    level: HeadingLevel,
    text: String,
) {
    // Pop headings at same or deeper level
    while stack.last().is_some_and(|(l, _)| *l >= level) {
        stack.pop();
    }
    stack.push((level, text));
}

fn breadcrumb(stack: &[(HeadingLevel, String)]) -> String {
    stack
        .iter()
        .map(|(level, text)| {
            let prefix = "#".repeat(heading_level_to_u8(*level) as usize);
            format!("{} {}", prefix, text)
        })
        .collect::<Vec<_>>()
        .join(" > ")
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn byte_offset_to_line(content: &str, offset: usize) -> usize {
    let clamped = offset.min(content.len());
    content[..clamped].chars().filter(|&c| c == '\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn chunk(content: &str) -> ChunkedFile {
        chunk_markdown(content, &PathBuf::from("test.md")).unwrap()
    }

    #[test]
    fn test_basic_heading_chunking() {
        let content = "# H1\nbody1\n## H2\nbody2\n";
        let result = chunk(content);
        assert_eq!(result.chunks.len(), 2, "should produce 2 chunks");
        assert_eq!(result.chunks[0].heading_breadcrumb, "# H1");
        assert_eq!(result.chunks[0].body.trim(), "body1");
        assert_eq!(result.chunks[1].heading_breadcrumb, "# H1 > ## H2");
        assert_eq!(result.chunks[1].body.trim(), "body2");
    }

    #[test]
    fn test_no_headings() {
        let content = "Just some plain text.\nAnother paragraph.\n";
        let result = chunk(content);
        assert_eq!(result.chunks.len(), 1, "no-heading file should produce 1 chunk");
        assert_eq!(result.chunks[0].heading_level, 0);
        assert_eq!(result.chunks[0].heading_breadcrumb, "");
        assert!(result.chunks[0].body.contains("Just some plain text."));
    }

    #[test]
    fn test_frontmatter_extraction() {
        let content = "---\ntags:\n  - test\n  - notes\n---\n# Title\nBody text\n";
        let result = chunk(content);
        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.frontmatter.tags, vec!["test", "notes"]);
        assert!(!result.chunks[0].body.contains("tags"), "frontmatter should not appear in chunk body");
    }

    #[test]
    fn test_frontmatter_only() {
        let content = "---\ntags:\n  - test\n---\n";
        let result = chunk(content);
        assert_eq!(result.chunks.len(), 0, "frontmatter-only file should produce 0 chunks");
        assert_eq!(result.frontmatter.tags, vec!["test"]);
    }

    #[test]
    fn test_multi_event_heading() {
        let content = "## Hello **world**\nBody here\n";
        let result = chunk(content);
        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].heading_breadcrumb, "## Hello world");
    }

    #[test]
    fn test_nested_heading_breadcrumbs() {
        let content = "# A\ntext a\n## B\ntext b\n### C\ntext c\n## D\ntext d\n";
        let result = chunk(content);
        assert_eq!(result.chunks.len(), 4);
        assert_eq!(result.chunks[0].heading_breadcrumb, "# A");
        assert_eq!(result.chunks[1].heading_breadcrumb, "# A > ## B");
        assert_eq!(result.chunks[2].heading_breadcrumb, "# A > ## B > ### C");
        // D pops B and C, so breadcrumb resets
        assert_eq!(result.chunks[3].heading_breadcrumb, "# A > ## D");
    }

    #[test]
    fn test_content_before_first_heading() {
        let content = "Some intro text\n# First heading\nHeading body\n";
        let result = chunk(content);
        assert_eq!(result.chunks.len(), 2);
        assert_eq!(result.chunks[0].heading_level, 0);
        assert_eq!(result.chunks[0].heading_breadcrumb, "");
        assert!(result.chunks[0].body.contains("Some intro text"));
        assert_eq!(result.chunks[1].heading_breadcrumb, "# First heading");
    }

    #[test]
    fn test_empty_sections() {
        let content = "## A\n## B\nSome body\n";
        let result = chunk(content);
        // A has no body so it may be skipped or have empty body
        // B should have body
        let non_empty: Vec<_> = result.chunks.iter().filter(|c| !c.body.trim().is_empty()).collect();
        assert!(non_empty.len() >= 1, "at least B should have a chunk with body");
        let b_chunk = non_empty.last().unwrap();
        assert!(b_chunk.body.contains("Some body"));
    }

    #[test]
    fn test_malformed_frontmatter() {
        let content = "---\n{invalid yaml\n---\n# Title\nContent\n";
        let result = chunk(content);
        // Should not panic, should use default frontmatter
        assert!(result.chunks.len() >= 1, "content should still be chunked");
        assert!(result.chunks.iter().any(|c| c.body.contains("Content")));
    }

    #[test]
    fn test_deeply_nested_headings() {
        let content = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\nDeep body\n";
        let result = chunk(content);
        let last = result.chunks.last().unwrap();
        assert_eq!(
            last.heading_breadcrumb,
            "# H1 > ## H2 > ### H3 > #### H4 > ##### H5 > ###### H6"
        );
    }
}
