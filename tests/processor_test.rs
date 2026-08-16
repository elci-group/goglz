//! `DocumentProcessor`: size-limit skip logic and debounce coalescing.
mod support;

use goglz::monitor::{FileEvent, FileEventType};
use goglz::processor::{DocumentProcessor, ProcessingStatus};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

fn new_processor(max_file_size_mb: u64) -> (DocumentProcessor, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let config = support::test_config(dir.path().join("out"), max_file_size_mb);
    let ai_client = support::test_ai_client(&config);
    let (_tx, rx) = mpsc::unbounded_channel();
    (DocumentProcessor::new(config, ai_client, rx), dir)
}

// ---- size-limit skip logic ---------------------------------------------

#[tokio::test]
async fn oversized_file_is_skipped_not_processed() {
    let (processor, dir) = new_processor(1); // 1 MB limit
    let path = dir.path().join("huge.md");
    // 2 MiB, comfortably over the 1 MB limit (size check truncates to whole MB).
    std::fs::write(&path, vec![b'a'; 2 * 1024 * 1024]).unwrap();

    let result = processor.process_file(&path).await.expect("size check must not error");

    match result.status {
        ProcessingStatus::Skipped(reason) => {
            assert!(reason.contains("too large") || reason.contains("MB"), "unexpected reason: {reason}");
        }
        other => panic!("expected Skipped, got {other:?}"),
    }
    assert!(result.conceptualization.is_none());
    assert!(result.clarity_improvement.is_none());
}

#[tokio::test]
async fn file_under_the_limit_is_not_skipped() {
    let (processor, dir) = new_processor(10); // 10 MB limit
    let path = dir.path().join("small.md");
    std::fs::write(&path, "a small document").unwrap();

    // Even though the AI endpoint is unreachable, conceptualize/clarity calls
    // for md/txt/rst/asciidoc files use `.ok()` internally, so failures are
    // swallowed and processing still completes rather than skipping.
    let result = processor.process_file(&path).await.expect("must not error");
    assert!(
        matches!(result.status, ProcessingStatus::Completed),
        "expected Completed, got {:?}",
        result.status
    );
}

#[tokio::test]
async fn file_exactly_at_limit_boundary_is_not_skipped() {
    // size_mb = len / (1024*1024) truncates, so a file of exactly 1 MiB
    // yields size_mb == 1 which is NOT > 1, so it must not be skipped.
    let (processor, dir) = new_processor(1);
    let path = dir.path().join("boundary.md");
    std::fs::write(&path, vec![b'a'; 1024 * 1024]).unwrap();

    let result = processor.process_file(&path).await.expect("must not error");
    assert!(matches!(result.status, ProcessingStatus::Completed));
}

#[tokio::test]
async fn non_document_extension_propagates_ai_failure_as_error() {
    // Files outside the txt/md/rst/asciidoc match arm go through
    // `improve_clarity_with_llama(..).await?` directly (no `.ok()`), so an
    // unreachable AI endpoint must surface as an `Err`, not be swallowed.
    let (processor, dir) = new_processor(10);
    let path = dir.path().join("legacy.doc");
    std::fs::write(&path, "legacy content").unwrap();

    let result = processor.process_file(&path).await;
    assert!(result.is_err(), "expected AI failure to propagate as Err");
}

// ---- debounce coalescing ------------------------------------------------

#[tokio::test]
async fn repeated_events_for_same_path_coalesce_into_one_pending_entry() {
    let (mut processor, dir) = new_processor(10);
    let path = dir.path().join("notes.md");
    let debounce = Duration::from_millis(2000);

    for _ in 0..5 {
        processor
            .handle_file_event(
                FileEvent {
                    path: path.clone(),
                    event_type: FileEventType::Modified,
                },
                debounce,
            )
            .await;
    }

    assert_eq!(
        processor.pending_count(),
        1,
        "5 rapid events for the same file must coalesce into 1 pending entry, not queue duplicates"
    );
}

#[tokio::test]
async fn events_for_different_paths_stay_separate() {
    let (mut processor, dir) = new_processor(10);
    let debounce = Duration::from_millis(2000);

    for name in ["a.md", "b.md", "c.md"] {
        processor
            .handle_file_event(
                FileEvent {
                    path: dir.path().join(name),
                    event_type: FileEventType::Created,
                },
                debounce,
            )
            .await;
    }

    assert_eq!(processor.pending_count(), 3);
}

#[tokio::test]
async fn deleted_event_removes_path_from_pending_queue() {
    let (mut processor, dir) = new_processor(10);
    let path: PathBuf = dir.path().join("notes.md");
    let debounce = Duration::from_millis(2000);

    processor
        .handle_file_event(
            FileEvent { path: path.clone(), event_type: FileEventType::Created },
            debounce,
        )
        .await;
    assert_eq!(processor.pending_count(), 1);

    processor
        .handle_file_event(
            FileEvent { path, event_type: FileEventType::Deleted },
            debounce,
        )
        .await;
    assert_eq!(processor.pending_count(), 0);
}
