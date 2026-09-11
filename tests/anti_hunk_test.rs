// SPDX-License-Identifier: MIT
//! Benchmark anti-hunking against the documents Ferret currently flags as
//! "large hunks" in the sibling `uni` project.

use goglz::anti_hunk::{apply_anti_hunking, max_section_body_lines};
use goglz::config::AntiHunkingConfig;
use std::path::PathBuf;

#[test]
fn flagged_uni_docs_fit_within_max_section_lines_after_anti_hunking(
) -> Result<(), Box<dyn std::error::Error>> {
    let config = AntiHunkingConfig::default();
    let threshold = config.max_section_lines;

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let uni_root = manifest
        .parent()
        .ok_or("goglz lives inside a parent workspace/project directory")?
        .join("uni");

    let docs = [
        "README.md",
        "docs/NEXT-SESSION.md",
        "docs/kaptaind-integration-progress.md",
        "docs/remediation-protocol.md",
    ];

    println!(
        "{:<55} {:>10} {:>10} {:>10}",
        "file", "before", "after", "threshold"
    );
    println!("{}", "-".repeat(90));

    let mut any_failed = false;
    for rel in docs {
        let path = uni_root.join(rel);
        let content = std::fs::read_to_string(&path)?;

        let before = max_section_body_lines(&content, &config);
        let ventilated = apply_anti_hunking(&content, &config);
        let after = max_section_body_lines(&ventilated, &config);

        println!("{:<55} {:>10} {:>10} {:>10}", rel, before, after, threshold);

        if after > threshold {
            eprintln!(
                "FAIL: {} still has a section body of {} lines (threshold: {})",
                rel, after, threshold
            );
            any_failed = true;
        }
    }

    assert!(
        !any_failed,
        "one or more flagged documents still exceed the anti-hunking threshold"
    );
    Ok(())
}
