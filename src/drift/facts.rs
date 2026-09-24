// SPDX-License-Identifier: MIT
//! Ground truth about a repository that documentation claims are checked
//! against: the current version, file line counts, the number of tests, and
//! how much non-documentation history has landed since a given date.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::{DirEntry, WalkDir};

/// Directory names never descended into when scanning a repository.
const SKIPPED_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "build",
    "vendor",
    "__pycache__",
];

pub(crate) fn is_skipped(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    name.starts_with('.') || (entry.file_type().is_dir() && SKIPPED_DIRS.contains(&name.as_ref()))
}

/// Files under `root`, excluding hidden and build/vendor directories.
pub(crate) fn walk_files(root: &Path) -> impl Iterator<Item = PathBuf> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_skipped(e))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
}

pub struct RepoFacts {
    pub root: PathBuf,
    pub version: Option<String>,
    pub test_count: usize,
    is_git: bool,
}

impl RepoFacts {
    pub fn collect(root: &Path) -> Self {
        let is_git = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["rev-parse", "--is-inside-work-tree"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        Self {
            root: root.to_path_buf(),
            version: detect_version(root),
            test_count: count_tests(root),
            is_git,
        }
    }

    /// Line count of a repository-relative path, if it is a readable file.
    pub fn line_count(&self, rel: &str) -> Option<usize> {
        let path = self.root.join(rel);
        fs::read_to_string(path).ok().map(|s| s.lines().count())
    }

    /// Commits touching anything other than Markdown since `date`
    /// (`YYYY-MM-DD`). `None` outside a git repository.
    pub fn code_commits_since(&self, date: &str) -> Option<usize> {
        if !self.is_git {
            return None;
        }
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(["rev-list", "--count", "--no-merges"])
            .arg(format!("--since={date}T23:59:59"))
            .args(["HEAD", "--", ".", ":(exclude)*.md"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }
}

/// The project's declared version: `VERSION`, then `Cargo.toml`
/// (`[package]` or `[workspace.package]`), `package.json`, `pyproject.toml`.
pub fn detect_version(root: &Path) -> Option<String> {
    if let Ok(raw) = fs::read_to_string(root.join("VERSION")) {
        let v = raw.trim().trim_start_matches('v');
        if looks_like_version(v) {
            return Some(v.to_string());
        }
    }
    if let Some(v) = toml_version(
        &root.join("Cargo.toml"),
        &[&["package"], &["workspace", "package"]],
    ) {
        return Some(v);
    }
    if let Ok(raw) = fs::read_to_string(root.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(v) = json.get("version").and_then(|v| v.as_str()) {
                if looks_like_version(v) {
                    return Some(v.to_string());
                }
            }
        }
    }
    toml_version(
        &root.join("pyproject.toml"),
        &[&["project"], &["tool", "poetry"]],
    )
}

fn toml_version(path: &Path, tables: &[&[&str]]) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let doc: toml::Value = raw.parse().ok()?;
    for table in tables {
        let mut node = &doc;
        let mut found = true;
        for key in *table {
            match node.get(key) {
                Some(next) => node = next,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if !found {
            continue;
        }
        if let Some(v) = node.get("version").and_then(|v| v.as_str()) {
            if looks_like_version(v) {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn looks_like_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() >= 2
        && parts.iter().take(3).all(|p| {
            let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
            !digits.is_empty()
        })
}

/// Counts test definitions: Rust `#[test]`-style attributes, Python
/// `def test_*` in test modules, and `it(`/`test(` calls in JS/TS spec files.
pub fn count_tests(root: &Path) -> usize {
    walk_files(root)
        .map(|path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let kind = match ext {
                "rs" => TestSyntax::Rust,
                "py" if name.starts_with("test_") || name.ends_with("_test.py") => {
                    TestSyntax::Python
                }
                "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs"
                    if name.contains(".test.") || name.contains(".spec.") =>
                {
                    TestSyntax::Js
                }
                _ => return 0,
            };
            fs::read_to_string(&path)
                .map(|src| count_in_source(&src, kind))
                .unwrap_or(0)
        })
        .sum()
}

#[derive(Clone, Copy)]
enum TestSyntax {
    Rust,
    Python,
    Js,
}

fn count_in_source(src: &str, kind: TestSyntax) -> usize {
    src.lines()
        .map(str::trim_start)
        .filter(|line| match kind {
            TestSyntax::Rust => {
                line.starts_with("#[test]")
                    || line.starts_with("#[tokio::test")
                    || line.starts_with("#[rstest")
                    || line.starts_with("#[async_std::test")
            }
            TestSyntax::Python => {
                line.starts_with("def test_") || line.starts_with("async def test_")
            }
            TestSyntax::Js => line.starts_with("it(") || line.starts_with("test("),
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_shapes() {
        assert!(looks_like_version("10.4.0"));
        assert!(looks_like_version("1.2"));
        assert!(looks_like_version("1.2.3-rc.1"));
        assert!(!looks_like_version("latest"));
        assert!(!looks_like_version("1"));
    }

    #[test]
    fn counts_rust_attributes_only_at_line_start() {
        let src =
            "#[test]\nfn a() {}\n    #[tokio::test]\nasync fn b() {}\n// #[test] in a comment\n";
        assert_eq!(count_in_source(src, TestSyntax::Rust), 2);
    }
}
