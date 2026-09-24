// SPDX-License-Identifier: MIT
//! `goglz drift`: find documentation whose factual claims no longer match the
//! repository it describes.
//!
//! Status documents — assessments, scorecards, roadmaps — state facts that
//! were true when written (a version, a date, "`src/main.rs` is 1,900 lines",
//! "231 passing tests") and silently rot as the code moves on. Drift checks
//! each such claim deterministically against the repository (no model call),
//! so it can run as a test hook or CI gate.
//!
//! Changelogs and release notes describe past states on purpose and are
//! skipped.

pub mod claims;
pub mod facts;

use crate::error::Result;
use claims::Claim;
use facts::RepoFacts;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Thresholds for deciding when a claim has drifted.
#[derive(Debug, Clone)]
pub struct DriftOptions {
    /// Relative difference tolerated between a claimed and actual count
    /// (`0.25` = 25%).
    pub tolerance: f64,
    /// Non-Markdown commits tolerated after a document's stated date.
    pub max_commits: usize,
    /// Repository-relative path prefixes to skip.
    pub exclude: Vec<String>,
}

impl Default for DriftOptions {
    fn default() -> Self {
        Self {
            tolerance: 0.25,
            max_commits: 25,
            exclude: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingKind {
    Version,
    Date,
    LineCount,
    MissingPath,
    TestCount,
    RawPayload,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub kind: FindingKind,
    /// 1-based line of the claim; `None` for whole-document findings.
    pub line: Option<usize>,
    pub claimed: String,
    pub actual: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentDrift {
    /// Path relative to the repository root.
    pub path: PathBuf,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DriftReport {
    pub root: PathBuf,
    pub version: Option<String>,
    pub test_count: usize,
    pub documents_checked: usize,
    pub drifted: Vec<DocumentDrift>,
}

impl DriftReport {
    pub fn is_clean(&self) -> bool {
        self.drifted.is_empty()
    }
}

/// Changelog-style documents record history, so their facts are expected to
/// be "stale".
fn is_historical(rel: &Path) -> bool {
    let name = rel
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_default();
    [
        "CHANGELOG",
        "CHANGES",
        "HISTORY",
        "RELEASE_NOTES",
        "RELEASES",
        "NEWS",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
        || rel
            .components()
            .any(|c| matches!(c.as_os_str().to_str(), Some("releases" | "changelog")))
}

fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("md" | "markdown")
    )
}

fn relative_diff(claimed: usize, actual: usize) -> f64 {
    if actual == 0 {
        return if claimed == 0 { 0.0 } else { f64::INFINITY };
    }
    (claimed as f64 - actual as f64).abs() / actual as f64
}

/// Compare dotted versions component-wise, treating a shorter claim
/// ("9.6") as matching any version it prefixes ("9.6.3").
fn versions_match(claimed: &str, actual: &str) -> bool {
    let core = |v: &str| -> Vec<String> {
        v.split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(str::to_string)
            .collect()
    };
    let (c, a) = (core(claimed), core(actual));
    c.len() <= a.len() && c.iter().zip(&a).all(|(x, y)| x == y)
}

fn check_document(text: &str, facts: &RepoFacts, options: &DriftOptions) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut max_tests: Option<(usize, usize)> = None;

    for claim in claims::extract(text) {
        match claim {
            Claim::RawPayload => findings.push(Finding {
                kind: FindingKind::RawPayload,
                line: None,
                claimed: "document".into(),
                actual: "serialized model response".into(),
                message: "file is a JSON-wrapped model response, not a document; restore it from history or a backup".into(),
            }),
            Claim::Version { line, value } => {
                if let Some(actual) = &facts.version {
                    if !versions_match(&value, actual) {
                        findings.push(Finding {
                            kind: FindingKind::Version,
                            line: Some(line),
                            message: format!("describes version {value}; repository is at {actual}"),
                            claimed: value,
                            actual: actual.clone(),
                        });
                    }
                }
            }
            Claim::Date { line, value } => {
                if let Some(commits) = facts.code_commits_since(&value) {
                    if commits > options.max_commits {
                        findings.push(Finding {
                            kind: FindingKind::Date,
                            line: Some(line),
                            message: format!(
                                "dated {value}; {commits} non-documentation commits have landed since"
                            ),
                            claimed: value,
                            actual: format!("{commits} commits since"),
                        });
                    }
                }
            }
            Claim::LineCount { line, path, value } => match facts.line_count(&path) {
                None => {
                    // Only a path that looks repository-relative (has a
                    // directory component) is worth reporting as missing.
                    if path.contains('/') {
                        findings.push(Finding {
                            kind: FindingKind::MissingPath,
                            line: Some(line),
                            message: format!("sizes `{path}`, which no longer exists"),
                            claimed: path,
                            actual: "missing".into(),
                        });
                    }
                }
                Some(actual) if relative_diff(value, actual) > options.tolerance => {
                    findings.push(Finding {
                        kind: FindingKind::LineCount,
                        line: Some(line),
                        message: format!("says `{path}` is {value} lines; it is {actual}"),
                        claimed: value.to_string(),
                        actual: actual.to_string(),
                    });
                }
                Some(_) => {}
            },
            Claim::TestCount { line, value } => {
                if max_tests.is_none_or(|(_, v)| value > v) {
                    max_tests = Some((line, value));
                }
            }
        }
    }

    // A document's largest test figure is taken as its claimed total, so
    // "195 library tests plus 6 integration tests" is judged on 195, not 6.
    // Small figures describe individual changes, not the suite.
    if let Some((line, value)) = max_tests {
        let actual = facts.test_count;
        if value >= 10 && actual > 0 && relative_diff(value, actual) > options.tolerance {
            findings.push(Finding {
                kind: FindingKind::TestCount,
                line: Some(line),
                message: format!("cites {value} tests; the repository defines {actual}"),
                claimed: value.to_string(),
                actual: actual.to_string(),
            });
        }
    }

    findings.sort_by_key(|f| f.line.unwrap_or(0));
    findings
}

/// Scan every Markdown document under `root` for drifted claims.
pub fn scan(root: &Path, options: &DriftOptions) -> Result<DriftReport> {
    let root = root.canonicalize()?;
    let facts = RepoFacts::collect(&root);

    let mut docs: Vec<PathBuf> = facts::walk_files(&root)
        .filter(|p| is_markdown(p))
        .filter_map(|p| p.strip_prefix(&root).ok().map(Path::to_path_buf))
        .filter(|rel| !is_historical(rel))
        .filter(|rel| {
            let s = rel.to_string_lossy();
            !options
                .exclude
                .iter()
                .any(|prefix| s.starts_with(prefix.trim_start_matches("./")))
        })
        .collect();
    docs.sort();

    let mut drifted = Vec::new();
    for rel in &docs {
        // Non-UTF-8 files are not prose documents; skip rather than fail.
        let Ok(text) = fs::read_to_string(root.join(rel)) else {
            continue;
        };
        let findings = check_document(&text, &facts, options);
        if !findings.is_empty() {
            drifted.push(DocumentDrift {
                path: rel.clone(),
                findings,
            });
        }
    }

    Ok(DriftReport {
        version: facts.version.clone(),
        test_count: facts.test_count,
        documents_checked: docs.len(),
        root,
        drifted,
    })
}

/// Human-readable rendering of a report.
pub fn render(report: &DriftReport) -> String {
    let mut out = String::new();
    for doc in &report.drifted {
        out.push_str(&format!("{}  DRIFTED\n", doc.path.display()));
        for f in &doc.findings {
            match f.line {
                Some(line) => out.push_str(&format!("  L{line:<5} {}\n", f.message)),
                None => out.push_str(&format!("  {:<6} {}\n", "", f.message)),
            }
        }
    }
    out.push_str(&format!(
        "{} of {} document(s) drifted (repository version {}, {} tests)\n",
        report.drifted.len(),
        report.documents_checked,
        report.version.as_deref().unwrap_or("unknown"),
        report.test_count,
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_prefix_matches() {
        assert!(versions_match("9.6", "9.6.3"));
        assert!(versions_match("10.4.0", "10.4.0"));
        assert!(versions_match("1.2.3", "1.2.3-rc.1"));
        assert!(!versions_match("9.6.3", "10.4.0"));
        assert!(!versions_match("1.2.3", "1.2"));
    }

    #[test]
    fn historical_documents_are_skipped() {
        assert!(is_historical(Path::new("CHANGELOG.md")));
        assert!(is_historical(Path::new("docs/releases/v1.md")));
        assert!(!is_historical(Path::new("docs/ROADMAP.md")));
    }
}
