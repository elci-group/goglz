// SPDX-License-Identifier: MIT
//! Markdown section detection used by the anti-hunking post-processor.

#[derive(Debug)]
pub(crate) struct Section {
    pub(crate) heading: Option<String>,
    pub(crate) body: Vec<String>,
}

/// Split a line-oriented document into sections at the configured heading
/// prefixes. The first chunk before any matching heading is returned with
/// `heading: None`.
pub(crate) fn split_into_sections(lines: &[&str], headings: &[String]) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut current = Section {
        heading: None,
        body: Vec::new(),
    };

    for line in lines {
        if is_section_heading(line, headings).is_some() {
            sections.push(current);
            current = Section {
                heading: Some(line.to_string()),
                body: Vec::new(),
            };
        } else {
            current.body.push(line.to_string());
        }
    }
    sections.push(current);
    sections
}

/// Check whether a line is a section heading for one of the configured
/// prefixes. Longer prefixes are tried first so `###` wins over `##`.
pub(crate) fn is_section_heading(line: &str, headings: &[String]) -> Option<usize> {
    let mut ordered: Vec<&String> = headings.iter().collect();
    ordered.sort_by_key(|a| std::cmp::Reverse(a.len()));

    for prefix in ordered {
        if line.starts_with(prefix) {
            let rest = &line[prefix.len()..];
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                return Some(prefix.len());
            }
        }
    }
    None
}

/// Strip leading and trailing blank lines from a section body before it is
/// chunked. This avoids counting empty padding toward the line budget.
pub(crate) fn trim_surrounding_blank_lines(lines: &[String]) -> Vec<String> {
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(start);
    lines[start..end].to_vec()
}
