// SPDX-License-Identifier: MIT
use crate::anti_hunk::{apply_anti_hunking, max_section_body_lines};
use crate::config::AntiHunkingConfig;

fn default_config() -> AntiHunkingConfig {
    AntiHunkingConfig::default()
}

#[test]
fn disabled_returns_unchanged() {
    let config = AntiHunkingConfig {
        enabled: false,
        ..default_config()
    };
    let text = "# A\n\nlong body\n".repeat(200);
    assert_eq!(apply_anti_hunking(&text, &config), text);
}

#[test]
fn small_document_untouched() {
    let text = "# Title\n\nSome body.\n\n## Section\n\nShort.\n";
    assert_eq!(apply_anti_hunking(text, &default_config()), text);
}

#[test]
fn oversized_section_gets_part_headings() {
    let mut body = String::new();
    for i in 0..100 {
        body.push_str(&format!("Paragraph {}.\n\n", i));
    }
    let text = format!("## Section\n\n{}", body);
    let out = apply_anti_hunking(&text, &default_config());
    assert!(out.contains("### Part 1"));
    assert!(out.contains("### Part 2"));
    assert!(max_section_body_lines(&out, &default_config()) <= 80);
}

#[test]
fn respects_split_on_headings() {
    let mut body = String::new();
    for i in 0..50 {
        body.push_str(&format!("Line {}.\n", i));
    }
    let text = format!("# Title\n\n{}\n## A\n\nShort.\n", body);
    let config = AntiHunkingConfig {
        enabled: true,
        max_section_lines: 30,
        split_on_headings: vec!["##".to_string()],
    };
    let out = apply_anti_hunking(&text, &config);
    // With only "##" configured, the generated part headings must use that
    // same level so downstream measurement sees them as section boundaries.
    assert!(out.contains("## Part 1"));
    assert!(max_section_body_lines(&out, &config) <= 30);
}

#[test]
fn code_block_preserved_even_when_large() {
    let code: String = (0..100)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let text = format!("## Section\n\n```rust\n{}\n```\n", code);
    let out = apply_anti_hunking(&text, &default_config());
    assert!(out.contains("```rust"));
    assert!(out.contains("line 99"));
    // Every emitted code chunk must be a complete fenced block.
    assert!(out.matches("```").count().is_multiple_of(2));
}
