// SPDX-License-Identifier: MIT
//! `goglz drift` against real on-disk repositories: library-level checks via
//! `goglz::drift::scan`, and the CLI's exit-status contract via the binary.
use goglz::drift::{scan, DriftOptions, FindingKind};
use std::fs;
use std::path::Path;
use std::process::Command;

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn git(root: &Path, args: &[&str], date: &str) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// A repository at version 2.0.0 with a 40-line source file and 12 tests.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "VERSION", "2.0.0\n");
    write(root, "src/big.rs", &"// line\n".repeat(40));
    write(root, "tests/t.rs", &"#[test]\nfn t() {}\n".repeat(12));
    dir
}

fn kinds(report: &goglz::drift::DriftReport, doc: &str) -> Vec<FindingKind> {
    report
        .drifted
        .iter()
        .find(|d| d.path == Path::new(doc))
        .map(|d| d.findings.iter().map(|f| f.kind).collect())
        .unwrap_or_default()
}

#[test]
fn accurate_documents_are_clean() {
    let dir = fixture();
    write(
        dir.path(),
        "STATUS.md",
        "# Status\n**Version:** 2.0.0\n\n`src/big.rs` is 40 lines, with 12 passing tests.\n",
    );
    let report = scan(dir.path(), &DriftOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(report.version.as_deref(), Some("2.0.0"));
    assert_eq!(report.test_count, 12);
}

#[test]
fn stale_version_sizes_and_test_counts_are_reported() {
    let dir = fixture();
    write(
        dir.path(),
        "ASSESSMENT.md",
        "# Assessment\n**Version:** 1.4.0\n\n`src/big.rs` is around 1,200 lines.\n`src/gone.rs` is 300 lines.\nWe have 231 passing tests.\n",
    );
    let report = scan(dir.path(), &DriftOptions::default()).unwrap();
    assert_eq!(
        kinds(&report, "ASSESSMENT.md"),
        vec![
            FindingKind::Version,
            FindingKind::LineCount,
            FindingKind::MissingPath,
            FindingKind::TestCount
        ]
    );
}

#[test]
fn changelogs_and_excluded_paths_are_skipped() {
    let dir = fixture();
    let stale = "# Old\n**Version:** 0.1.0\n";
    write(dir.path(), "CHANGELOG.md", stale);
    write(dir.path(), "docs/releases/v0.1.md", stale);
    write(dir.path(), "docs/archive/old.md", stale);
    write(dir.path(), "target/doc.md", stale);
    let options = DriftOptions {
        exclude: vec!["docs/archive".into()],
        ..DriftOptions::default()
    };
    let report = scan(dir.path(), &options).unwrap();
    assert!(report.is_clean(), "{report:?}");
}

#[test]
fn dated_documents_drift_after_enough_code_commits() {
    let dir = fixture();
    let root = dir.path();
    write(root, "PLAN.md", "# Plan\n**Date:** 2026-01-01\n");
    git(root, &["init", "-q"], "2026-01-01T12:00:00");
    git(root, &["add", "."], "2026-01-01T12:00:00");
    git(root, &["commit", "-qm", "init"], "2026-01-01T12:00:00");
    for i in 0..3 {
        write(root, &format!("src/f{i}.rs"), "fn f() {}\n");
        git(root, &["add", "."], "2026-02-01T12:00:00");
        git(root, &["commit", "-qm", "code"], "2026-02-01T12:00:00");
    }
    // Documentation-only commits never count toward drift.
    write(root, "NOTES.md", "notes\n");
    git(root, &["add", "."], "2026-02-02T12:00:00");
    git(root, &["commit", "-qm", "docs"], "2026-02-02T12:00:00");

    let lenient = scan(root, &DriftOptions::default()).unwrap();
    assert!(lenient.is_clean(), "{lenient:?}");

    let strict = DriftOptions {
        max_commits: 2,
        ..DriftOptions::default()
    };
    let report = scan(root, &strict).unwrap();
    assert_eq!(kinds(&report, "PLAN.md"), vec![FindingKind::Date]);
    assert!(report.drifted[0].findings[0]
        .message
        .contains("3 non-documentation commits"));
}

#[test]
fn raw_model_payload_is_reported() {
    let dir = fixture();
    write(
        dir.path(),
        "README.md",
        "{\n  \"content\": \"# Title\\n\\ntrunc",
    );
    let report = scan(dir.path(), &DriftOptions::default()).unwrap();
    assert_eq!(kinds(&report, "README.md"), vec![FindingKind::RawPayload]);
}

#[test]
fn cli_strict_exit_status_and_json() {
    let home = tempfile::tempdir().unwrap();
    let dir = fixture();
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_goglz"))
            .env("HOME", home.path())
            .args(["drift", "--directory"])
            .arg(dir.path())
            .args(extra)
            .output()
            .unwrap()
    };

    write(dir.path(), "OK.md", "# Ok\n**Version:** 2.0.0\n");
    assert!(run(&["--strict"]).status.success());

    write(dir.path(), "OLD.md", "# Old\n**Version:** 1.0.0\n");
    let out = run(&["--strict"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("OLD.md  DRIFTED"));

    // Without --strict, drift is reported but not a failure.
    let out = run(&["--format", "json"]);
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["drifted"][0]["path"], "OLD.md");
    assert_eq!(json["drifted"][0]["findings"][0]["kind"], "version");

    let out = run(&["--tolerance", "-1"]);
    assert!(!out.status.success());
}
