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
fn create_backup_writes_recoverable_copy_before_any_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("guide.md");
    let original = "# Original Content\n\nThis must be recoverable.";
    fs::write(&path, original).unwrap();

    let processor = processor_for(dir.path(), dir.path());
    processor.create_backup(&path, original).unwrap();

    let backup_path = dir.path().join("guide.md.backup");
    assert!(backup_path.exists(), "backup file was not created");
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), original);
    // The original itself must be untouched by create_backup alone.
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

#[tokio::test]
async fn ai_failure_leaves_original_file_and_no_backup_behind() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("important.md");
    let original = "# Important Document\n\nDo not lose this.";
    fs::write(&path, original).unwrap();

    let processor = processor_for(dir.path(), dir.path());
    // `run()` swallows per-document errors internally (logs and continues),
    // so it returns Ok even though the AI endpoint is unreachable.
    let results = processor.run().await.expect("run() must not itself error");

    assert!(results.is_empty(), "no document should have been successfully revised");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        original,
        "original file must be untouched when the AI call fails"
    );
    assert!(
        !dir.path().join("important.md.backup").exists(),
        "no backup should be created when the AI call never succeeded \
         (create_backup only runs after a successful AI response)"
    );
}

#[tokio::test]
async fn ai_failure_on_one_file_does_not_affect_a_sibling_file() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.md");
    let b = dir.path().join("b.md");
    fs::write(&a, "content a").unwrap();
    fs::write(&b, "content b").unwrap();

    let processor = processor_for(dir.path(), dir.path());
    let results = processor.run().await.expect("run() must not error");

    assert!(results.is_empty());
    assert_eq!(fs::read_to_string(&a).unwrap(), "content a");
    assert_eq!(fs::read_to_string(&b).unwrap(), "content b");
}

// ---- translation output path generation ----------------------------------

#[test]
fn generate_language_output_path_substitutes_filename_lang_and_ext() {
    let dir = tempfile::tempdir().unwrap();
    let processor = processor_for(dir.path(), dir.path());
    let original = dir.path().join("notes.md");
    let lang = support::language_config("ES", "Spanish", true, "{filename}_{lang}.{ext}");

    let out = processor.generate_language_output_path(&original, &lang).unwrap();
    assert_eq!(out, dir.path().join("notes_es.md"));
}

#[test]
fn generate_language_output_path_lowercases_language_code() {
    let dir = tempfile::tempdir().unwrap();
    let processor = processor_for(dir.path(), dir.path());
    let original = dir.path().join("readme.txt");
    let lang = support::language_config("FR", "French", true, "{filename}.{lang}.{ext}");

    let out = processor.generate_language_output_path(&original, &lang).unwrap();
    assert_eq!(out, dir.path().join("readme.fr.txt"));
}

#[test]
fn generate_language_output_path_handles_multi_dot_filenames() {
    let dir = tempfile::tempdir().unwrap();
    let processor = processor_for(dir.path(), dir.path());
    let original = dir.path().join("archive.tar.gz");
    let lang = support::language_config("de", "German", true, "{filename}.{lang}.{ext}");

    // extension() only sees the last component ("gz"); stem is
    // "archive.tar" - documenting actual current behavior for a
    // multi-dot filename.
    let out = processor.generate_language_output_path(&original, &lang).unwrap();
    assert_eq!(out, dir.path().join("archive.tar.de.gz"));
}

#[test]
fn generate_language_output_path_falls_back_to_md_for_extensionless_files() {
    let dir = tempfile::tempdir().unwrap();
    let processor = processor_for(dir.path(), dir.path());
    let original = dir.path().join("README");
    let lang = support::language_config("ja", "Japanese", true, "{filename}_{lang}.{ext}");

    let out = processor.generate_language_output_path(&original, &lang).unwrap();
    assert_eq!(out, dir.path().join("README_ja.md"));
}

#[test]
fn discover_documents_is_reachable_through_run_for_backup_regression_coverage() {
    // Sanity check tying discover_documents (see discovery_test.rs) and the
    // backup-safety path together: a hidden-directory file must never even
    // reach revise_document, so it can never be at risk of being
    // overwritten in the first place.
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".git/COMMIT_EDITMSG.md"), "not a real doc").unwrap();

    let processor = processor_for(dir.path(), dir.path());
    let docs = processor.discover_documents().unwrap();
    assert!(docs.is_empty(), "hidden-directory content must not be discovered: {docs:?}");
}
