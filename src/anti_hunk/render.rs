// SPDX-License-Identifier: MIT
//! Render a markdown section, inserting part headings when it exceeds the
//! configured line budget.

use crate::anti_hunk::chunker;
use crate::anti_hunk::section::{trim_surrounding_blank_lines, Section};

pub(crate) fn emit_section(
    out: &mut Vec<String>,
    section: Section,
    max_section_lines: usize,
    split_on_headings: &[String],
) {
    if let Some(ref heading) = section.heading {
        out.push(heading.clone());
    }

    // Keep small sections exactly as the model wrote them so we don't disturb
    // intentional spacing around headings.
    if section.body.len() <= max_section_lines {
        out.extend(section.body);
        return;
    }

    let budget = max_section_lines.saturating_sub(2).max(1);
    let body = trim_surrounding_blank_lines(&section.body);
    let chunks = chunker::chunk_body(&body, budget);
    let prefix = part_prefix(&section, split_on_headings);

    for (i, chunk) in chunks.iter().enumerate() {
        out.push(format!("{} Part {}", prefix, i + 1));
        for (j, para) in chunk.iter().enumerate() {
            if j > 0 {
                out.push(String::new());
            }
            out.extend(para.iter().cloned());
        }
    }
}

fn part_prefix(section: &Section, split_on_headings: &[String]) -> String {
    let base_level = section
        .heading
        .as_ref()
        .map(|h| h.chars().take_while(|c| *c == '#').count())
        .unwrap_or(0);

    let mut levels: Vec<usize> = split_on_headings.iter().map(|p| p.len()).collect();
    levels.sort();

    // Prefer a heading level deeper than the parent so the part headings look
    // subordinate. If none is configured, fall back to the deepest configured
    // level (or base + 1 as a last resort). The key invariant is that the
    // generated prefix must be recognised by `split_on_headings` so the chunk
    // becomes its own section in downstream measurement.
    let chosen = levels
        .iter()
        .find(|&&l| l > base_level)
        .copied()
        .or_else(|| levels.last().copied())
        .unwrap_or_else(|| (base_level + 1).min(6));

    "#".repeat(chosen)
}
