//! File discovery / pattern matching, covering both the live-monitor glob
//! matcher (`monitor::DirectoryMonitor`) and the `revise` walker
//! (`revise::ReviseProcessor::discover_documents`).
mod support;

use goglz::config::Config;
use goglz::monitor::DirectoryMonitor;
use goglz::revise::ReviseProcessor;
use notify::{Event, EventKind};
use notify::event::{CreateKind, ModifyKind, RemoveKind};
use std::fs;
use std::path::PathBuf;

// ---- monitor::DirectoryMonitor::matches_pattern -----------------------

#[test]
fn wildcard_star_matches_anything() {
    assert!(DirectoryMonitor::matches_pattern("anything.exe", "*"));
    assert!(DirectoryMonitor::matches_pattern("", "*"));
}

#[test]
fn extension_glob_matches_only_that_extension() {
    assert!(DirectoryMonitor::matches_pattern("notes.md", "*.md"));
    assert!(!DirectoryMonitor::matches_pattern("notes.txt", "*.md"));
    // Documents actual (loose) behavior: `*.md` strips to a plain suffix
    // check ("md"), not a literal ".md" match, so a name that merely ends
    // in the letters "md" without a dot still matches.
    assert!(DirectoryMonitor::matches_pattern("notesxmd", "*.md"));
    assert!(!DirectoryMonitor::matches_pattern("notes.mdx", "*.md"));
}

#[test]
fn prefix_and_suffix_glob_matches_both_ends() {
    assert!(DirectoryMonitor::matches_pattern("draft-1.md", "draft-*.md"));
    assert!(!DirectoryMonitor::matches_pattern("final-1.md", "draft-*.md"));
    assert!(!DirectoryMonitor::matches_pattern("draft-1.txt", "draft-*.md"));
}

#[test]
fn exact_pattern_requires_exact_match() {
    assert!(DirectoryMonitor::matches_pattern("README.md", "README.md"));
    assert!(!DirectoryMonitor::matches_pattern("README.md.bak", "README.md"));
}

// ---- monitor::DirectoryMonitor::process_event_static -------------------

fn notify_event(kind: EventKind, path: PathBuf) -> Event {
    Event::new(kind).add_path(path)
}

#[test]
fn process_event_static_matches_file_in_monitored_dir_with_pattern() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.md");
    fs::write(&file, "hello").unwrap();

    let mut config = Config::default();
    config.directories = vec![support::monitored_directory(
        dir.path().to_path_buf(),
        vec!["*.md"],
        true,
    )];

    let event = notify_event(EventKind::Create(CreateKind::File), file.clone());
    let result = DirectoryMonitor::process_event_static(event, &config);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.path, file);
    assert!(matches!(result.event_type, goglz::monitor::FileEventType::Created));
}

#[test]
fn process_event_static_ignores_non_matching_pattern() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.bin");
    fs::write(&file, "hello").unwrap();

    let mut config = Config::default();
    config.directories = vec![support::monitored_directory(
        dir.path().to_path_buf(),
        vec!["*.md"],
        true,
    )];

    let event = notify_event(EventKind::Modify(ModifyKind::Any), file);
    assert!(DirectoryMonitor::process_event_static(event, &config).is_none());
}

#[test]
fn process_event_static_ignores_paths_outside_monitored_dirs() {
    let monitored = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let file = other.path().join("notes.md");
    fs::write(&file, "hello").unwrap();

    let mut config = Config::default();
    config.directories = vec![support::monitored_directory(
        monitored.path().to_path_buf(),
        vec!["*.md"],
        true,
    )];

    let event = notify_event(EventKind::Create(CreateKind::File), file);
    assert!(DirectoryMonitor::process_event_static(event, &config).is_none());
}

#[test]
fn process_event_static_ignores_directories() {
    let dir = tempfile::tempdir().unwrap();
    let subdir = dir.path().join("notes.md"); // named like a file but IS a dir
    fs::create_dir(&subdir).unwrap();

    let mut config = Config::default();
    config.directories = vec![support::monitored_directory(
        dir.path().to_path_buf(),
        vec!["*.md"],
        true,
    )];

    let event = notify_event(EventKind::Create(CreateKind::Folder), subdir);
    assert!(DirectoryMonitor::process_event_static(event, &config).is_none());
}

#[test]
fn process_event_static_maps_remove_to_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.md");
    fs::write(&file, "hello").unwrap();

    let mut config = Config::default();
    config.directories = vec![support::monitored_directory(
        dir.path().to_path_buf(),
        vec!["*"],
        true,
    )];

    let event = notify_event(EventKind::Remove(RemoveKind::File), file);
    let result = DirectoryMonitor::process_event_static(event, &config).unwrap();
    assert!(matches!(result.event_type, goglz::monitor::FileEventType::Deleted));
}

// ---- revise::ReviseProcessor::discover_documents ------------------------

fn revise_processor_for(dir: &std::path::Path) -> ReviseProcessor {
    let config = support::test_config(dir.join("out"), 10);
    let ai_client = support::test_ai_client(&config);
    ReviseProcessor::new(
        ai_client,
        support::minimal_revise_config(),
        dir.to_path_buf(),
        Some(dir.to_path_buf()),
    )
}

#[test]
fn discover_documents_finds_supported_extensions_recursively() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("nested/deep")).unwrap();

    for (rel, content) in [
        ("notes.md", "# Notes"),
        ("readme.txt", "readme"),
        ("guide.rst", "guide"),
        ("spec.asciidoc", "spec"),
        ("spec2.adoc", "spec2"),
        ("legacy.doc", "legacy"),
        ("legacy2.docx", "legacy2"),
        ("nested/deep/doc.rst", "deep doc"),
    ] {
        fs::write(dir.path().join(rel), content).unwrap();
    }
    // Non-matching extensions must be excluded.
    fs::write(dir.path().join("data.bin"), [0u8, 1, 2]).unwrap();
    fs::write(dir.path().join("image.png"), [0u8, 1, 2]).unwrap();

    let processor = revise_processor_for(dir.path());
    let docs = processor.discover_documents().unwrap();

    let names: Vec<String> = docs
        .iter()
        .map(|p| p.strip_prefix(dir.path()).unwrap().to_string_lossy().to_string())
        .collect();

    for expected in [
        "notes.md",
        "readme.txt",
        "guide.rst",
        "spec.asciidoc",
        "spec2.adoc",
        "legacy.doc",
        "legacy2.docx",
        "nested/deep/doc.rst",
    ] {
        assert!(names.contains(&expected.to_string()), "missing {expected}: got {names:?}");
    }
    assert!(!names.iter().any(|n| n.contains("data.bin")));
    assert!(!names.iter().any(|n| n.contains("image.png")));
}

#[test]
fn discover_documents_skips_dotfiles_at_top_level() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".env"), "SECRET=1").unwrap();
    fs::write(dir.path().join(".hidden.md"), "hidden doc").unwrap();
    fs::write(dir.path().join("visible.md"), "visible doc").unwrap();

    let processor = revise_processor_for(dir.path());
    let docs = processor.discover_documents().unwrap();
    let names: Vec<String> = docs
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert_eq!(names, vec!["visible.md".to_string()]);
}

#[test]
fn discover_documents_prunes_hidden_directories_entirely() {
    // Regression test: files that live *inside* a hidden directory (like
    // `.git/`, `.github/`, or `.venv/`) must never be discovered, even
    // though the file's own name doesn't start with a dot. Before this was
    // fixed, `discover_documents` only skipped the directory *entry* itself
    // (`continue`), which does not stop WalkDir from still recursing into
    // it - so `.github/ISSUE_TEMPLATE/bug_report.md` was silently included
    // and would have been overwritten by `goglz revise`.
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".git/objects")).unwrap();
    fs::create_dir_all(dir.path().join(".github/ISSUE_TEMPLATE")).unwrap();
    fs::create_dir_all(dir.path().join(".venv/lib")).unwrap();
    fs::write(dir.path().join(".git/config"), "[core]").unwrap();
    fs::write(
        dir.path().join(".github/ISSUE_TEMPLATE/bug_report.md"),
        "template",
    )
    .unwrap();
    fs::write(dir.path().join(".venv/lib/README.txt"), "venv readme").unwrap();
    fs::write(dir.path().join("real-doc.md"), "the only real doc").unwrap();

    let processor = revise_processor_for(dir.path());
    let docs = processor.discover_documents().unwrap();
    let names: Vec<String> = docs
        .iter()
        .map(|p| p.strip_prefix(dir.path()).unwrap().to_string_lossy().to_string())
        .collect();

    assert_eq!(names, vec!["real-doc.md".to_string()], "hidden-dir contents leaked into discovery: {names:?}");
}

#[test]
fn discover_documents_on_empty_directory_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let processor = revise_processor_for(dir.path());
    assert!(processor.discover_documents().unwrap().is_empty());
}
