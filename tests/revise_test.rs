// SPDX-License-Identifier: MIT
//! `ReviseProcessor`: backup safety and translation output path generation.
//!
//! This is the single most safety-critical path in goglz: `revise` performs
//! an ungated, no-dry-run, in-place overwrite of every discovered doc file,
//! backed only by one `.backup` copy. These tests lock in the current (and
//! correct) ordering inside `revise_document`: call the AI *first*, only
//! create a backup and overwrite the original *after* that call succeeds. If
//! a future change reorders those steps (e.g. writes before the AI call
//! resolves, or drops the backup step), these tests should catch it.
mod support;

use goglz::revise::ReviseProcessor;
use std::error::Error;
use std::fs;

fn processor_for(target_dir: &std::path::Path, project_root: &std::path::Path) -> ReviseProcessor {
    let config = support::test_config(target_dir.join("out"), 10);
    let ai_client = support::test_ai_client(&config);
    ReviseProcessor::new(
        ai_client,
        support::minimal_revise_config(),
        project_root.to_path_buf(),
        Some(target_dir.to_path_buf()),
    )
}

// ---- backup safety -------------------------------------------------------

#[test]
fn create_backup_writes_recoverable_copy_before_any_overwrite() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("guide.md");
    let original = "# Original Content\n\nThis must be recoverable.";
    fs::write(&path, original)?;

    let processor = processor_for(dir.path(), dir.path());
    processor.create_backup(&path, original)?;

    let backup_path = dir.path().join("guide.md.backup");
    assert!(backup_path.exists(), "backup file was not created");
    assert_eq!(fs::read_to_string(&backup_path)?, original);
    // The original itself must be untouched by create_backup alone.
    assert_eq!(fs::read_to_string(&path)?, original);
    Ok(())
}

#[tokio::test]
async fn ai_failure_leaves_original_file_and_no_backup_behind() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("important.md");
    let original = "# Important Document\n\nDo not lose this.";
    fs::write(&path, original)?;

    let processor = processor_for(dir.path(), dir.path());
    // `run()` swallows per-document errors internally (logs and continues),
    // so it returns Ok even though the AI endpoint is unreachable.
    let results = processor.run().await?;

    assert!(
        results.is_empty(),
        "no document should have been successfully revised"
    );
    assert_eq!(
        fs::read_to_string(&path)?,
        original,
        "original file must be untouched when the AI call fails"
    );
    assert!(
        !dir.path().join("important.md.backup").exists(),
        "no backup should be created when the AI call never succeeded \
         (create_backup only runs after a successful AI response)"
    );
    Ok(())
}

#[tokio::test]
async fn ai_failure_on_one_file_does_not_affect_a_sibling_file() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let a = dir.path().join("a.md");
    let b = dir.path().join("b.md");
    fs::write(&a, "content a")?;
    fs::write(&b, "content b")?;

    let processor = processor_for(dir.path(), dir.path());
    let results = processor.run().await?;

    assert!(results.is_empty());
    assert_eq!(fs::read_to_string(&a)?, "content a");
    assert_eq!(fs::read_to_string(&b)?, "content b");
    Ok(())
}

// ---- translation output path generation ----------------------------------

#[test]
fn generate_language_output_path_substitutes_filename_lang_and_ext() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let processor = processor_for(dir.path(), dir.path());
    let original = dir.path().join("notes.md");
    let lang = support::language_config("ES", "Spanish", true, "{filename}_{lang}.{ext}");

    let out = processor.generate_language_output_path(&original, &lang)?;
    assert_eq!(out, dir.path().join("notes_es.md"));
    Ok(())
}

#[test]
fn generate_language_output_path_lowercases_language_code() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let processor = processor_for(dir.path(), dir.path());
    let original = dir.path().join("readme.txt");
    let lang = support::language_config("FR", "French", true, "{filename}.{lang}.{ext}");

    let out = processor.generate_language_output_path(&original, &lang)?;
    assert_eq!(out, dir.path().join("readme.fr.txt"));
    Ok(())
}

#[test]
fn generate_language_output_path_handles_multi_dot_filenames() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let processor = processor_for(dir.path(), dir.path());
    let original = dir.path().join("archive.tar.gz");
    let lang = support::language_config("de", "German", true, "{filename}.{lang}.{ext}");

    // extension() only sees the last component ("gz"); stem is
    // "archive.tar" - documenting actual current behavior for a
    // multi-dot filename.
    let out = processor.generate_language_output_path(&original, &lang)?;
    assert_eq!(out, dir.path().join("archive.tar.de.gz"));
    Ok(())
}

#[test]
fn generate_language_output_path_falls_back_to_md_for_extensionless_files(
) -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let processor = processor_for(dir.path(), dir.path());
    let original = dir.path().join("README");
    let lang = support::language_config("ja", "Japanese", true, "{filename}_{lang}.{ext}");

    let out = processor.generate_language_output_path(&original, &lang)?;
    assert_eq!(out, dir.path().join("README_ja.md"));
    Ok(())
}

#[test]
fn discover_documents_is_reachable_through_run_for_backup_regression_coverage(
) -> Result<(), Box<dyn Error>> {
    // Sanity check tying discover_documents (see discovery_test.rs) and the
    // backup-safety path together: a hidden-directory file must never even
    // reach revise_document, so it can never be at risk of being
    // overwritten in the first place.
    let dir = tempfile::tempdir()?;
    fs::create_dir_all(dir.path().join(".git"))?;
    fs::write(dir.path().join(".git/COMMIT_EDITMSG.md"), "not a real doc")?;

    let processor = processor_for(dir.path(), dir.path());
    let docs = processor.discover_documents();
    assert!(
        docs.is_empty(),
        "hidden-directory content must not be discovered: {docs:?}"
    );
    Ok(())
}
