// SPDX-License-Identifier: MIT
//! "Scrawny" anti-hunking post-processor for markdown documents.
//!
//! Downstream assessment tools such as Ferret flag documents that contain long
//! contiguous blocks of non-heading text as "large hunks". This module
//! ventilates revised documents by splitting oversized markdown sections at
//! paragraph boundaries and inserting explicit sub-headings.

mod chunker;
mod render;
mod section;
mod split;

#[cfg(test)]
mod tests;

use crate::config::AntiHunkingConfig;

/// Ventilate `content` according to `config`.
///
/// If anti-hunking is disabled, the input is returned unchanged. Otherwise the
/// document is split into sections at the configured heading levels and any
/// section whose body exceeds `max_section_lines` is broken into smaller
/// chunks separated by generated part headings.
pub fn apply_anti_hunking(content: &str, config: &AntiHunkingConfig) -> String {
    if !config.enabled || config.max_section_lines == 0 || content.is_empty() {
        return content.to_string();
    }

    let ends_with_newline = content.ends_with('\n');
    let lines: Vec<&str> = content.lines().collect();
    let sections = section::split_into_sections(&lines, &config.split_on_headings);

    let mut out: Vec<String> = Vec::with_capacity(lines.len() + sections.len() * 2);
    for section in sections {
        render::emit_section(
            &mut out,
            section,
            config.max_section_lines,
            &config.split_on_headings,
        );
    }

    let mut result = out.join("\n");
    if ends_with_newline && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Return the largest section body (in lines) found in `content` using the
/// same heading markers that `apply_anti_hunking` uses.
pub fn max_section_body_lines(content: &str, config: &AntiHunkingConfig) -> usize {
    if !config.enabled || config.max_section_lines == 0 {
        return 0;
    }
    let lines: Vec<&str> = content.lines().collect();
    let sections = section::split_into_sections(&lines, &config.split_on_headings);
    sections.iter().map(|s| s.body.len()).max().unwrap_or(0)
}
