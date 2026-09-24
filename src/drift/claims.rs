// SPDX-License-Identifier: MIT
//! Extraction of checkable factual claims from a Markdown document.
//!
//! Only prose is inspected: fenced code blocks are skipped, so example
//! configs (`version = "1.2.3"`) and command output never count as claims.
//! Version and date stamps are only read from a document's header (its first
//! [`HEADER_LINES`] prose lines), where status documents put them.

use regex::Regex;
use std::sync::OnceLock;

/// How many leading prose lines count as a document's metadata header.
pub const HEADER_LINES: usize = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claim {
    /// "**Version:** 9.6.3" in the header.
    Version { line: usize, value: String },
    /// "**Date:** 2026-06-04" / "as of 2026-06-04" in the header.
    Date { line: usize, value: String },
    /// "`src/cli/main.rs` is around 1,951 lines".
    LineCount {
        line: usize,
        path: String,
        value: usize,
    },
    /// "231 passing tests".
    TestCount { line: usize, value: usize },
    /// The file is a serialized model response (`{"content": "..."}`)
    /// rather than a document.
    RawPayload,
}

fn version_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\bversion\b[*_]*\s*[:=][*_]*\s*`?v?(\d+\.\d+(?:\.\d+)?)").unwrap()
    })
}

fn date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?:date|as of|last updated|updated|generated|assessed)\b[*_]*\s*[:=]?[*_]*\s*(\d{4}-\d{2}-\d{2})\b",
        )
        .unwrap()
    })
}

fn line_count_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"`([A-Za-z0-9_./-]+\.[A-Za-z0-9]+)`[^`\n]{0,40}?\b(\d{1,3}(?:,\d{3})+|\d{2,})\s*(?:lines|LOC|loc|SLOC)\b",
        )
        .unwrap()
    })
}

fn test_count_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(\d{1,3}(?:,\d{3})+|\d+)\s+(?:(?:passing|green|unit|library|lib|integration|automated)\s+)*tests\b",
        )
        .unwrap()
    })
}

fn parse_number(raw: &str) -> Option<usize> {
    raw.replace(',', "").parse().ok()
}

/// True when the document is a JSON object wrapping the real text, which is
/// what a model response written straight to disk looks like.
fn is_raw_payload(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('{') && {
        let head: String = trimmed.chars().take(200).collect();
        head.contains("\"content\"") || head.contains("\"text\"") || head.contains("\"choices\"")
    }
}

pub fn extract(text: &str) -> Vec<Claim> {
    if is_raw_payload(text) {
        return vec![Claim::RawPayload];
    }

    let mut claims = Vec::new();
    let mut in_fence = false;
    let mut prose_lines = 0usize;

    for (idx, line) in text.lines().enumerate() {
        let lineno = idx + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        prose_lines += 1;

        if prose_lines <= HEADER_LINES {
            if let Some(c) = version_re().captures(line) {
                claims.push(Claim::Version {
                    line: lineno,
                    value: c[1].to_string(),
                });
            }
            if let Some(c) = date_re().captures(line) {
                claims.push(Claim::Date {
                    line: lineno,
                    value: c[1].to_string(),
                });
            }
        }

        for c in line_count_re().captures_iter(line) {
            if let Some(value) = parse_number(&c[2]) {
                claims.push(Claim::LineCount {
                    line: lineno,
                    path: c[1].to_string(),
                    value,
                });
            }
        }
        for c in test_count_re().captures_iter(line) {
            if let Some(value) = parse_number(&c[1]) {
                claims.push(Claim::TestCount {
                    line: lineno,
                    value,
                });
            }
        }
    }
    claims
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_version_and_date() {
        let doc = "# Report\n**Date:** 2026-06-04 | **Version:** 9.6.3 | **Scope:** all\n";
        let claims = extract(doc);
        assert!(claims.contains(&Claim::Version {
            line: 2,
            value: "9.6.3".into()
        }));
        assert!(claims.contains(&Claim::Date {
            line: 2,
            value: "2026-06-04".into()
        }));
    }

    #[test]
    fn version_outside_header_is_ignored() {
        let mut doc = String::from("# T\n");
        for _ in 0..HEADER_LINES {
            doc.push_str("prose\n");
        }
        doc.push_str("Version: 1.0.0\n");
        assert!(extract(&doc).is_empty());
    }

    #[test]
    fn fenced_code_is_ignored() {
        let doc = "# T\n```toml\nversion = \"1.2.3\"\n```\n";
        assert!(extract(doc).is_empty());
    }

    #[test]
    fn line_and_test_counts() {
        let doc = "`src/cli/main.rs` is around 1,951 lines.\nRust has 195 library tests plus 6 integration tests passing.\n";
        let claims = extract(doc);
        assert!(claims.contains(&Claim::LineCount {
            line: 1,
            path: "src/cli/main.rs".into(),
            value: 1951
        }));
        assert!(claims.contains(&Claim::TestCount {
            line: 2,
            value: 195
        }));
        assert!(claims.contains(&Claim::TestCount { line: 2, value: 6 }));
    }

    #[test]
    fn raw_payload_detected() {
        assert_eq!(
            extract("{\n  \"content\": \"# Title\\n"),
            vec![Claim::RawPayload]
        );
        assert!(!extract("{{ template }}\n# Doc").contains(&Claim::RawPayload));
    }
}
