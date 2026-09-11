// SPDX-License-Identifier: MIT
//! `DocumentProcessor`: size-limit skip logic and debounce coalescing.
mod support;

use goglz::monitor::{FileEvent, FileEventType};
use goglz::processor::{DocumentProcessor, ProcessingStatus};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

fn skip_reason(status: ProcessingStatus) -> Result<String, Box<dyn std::error::Error>> {
    match status {
        ProcessingStatus::Skipped(reason) => Ok(reason),
        other => Err(format!("expected Skipped, got {other:?}").into()),
    }
}

fn new_processor(
    max_file_size_mb: u64,
) -> Result<(DocumentProcessor, tempfile::TempDir), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let config = support::test_config(dir.path().join("out"), max_file_size_mb);
    let ai_client = support::test_ai_client(&config);
    let (_tx, rx) = mpsc::unbounded_channel();
    Ok((DocumentProcessor::new(config, ai_client, rx), dir))
}

// ---- size-limit skip logic ---------------------------------------------

#[tokio::test]
async fn oversized_file_is_skipped_not_processed() -> Result<(), Box<dyn std::error::Error>> {
    let (processor, dir) = new_processor(1)?; // 1 MB limit
    let path = dir.path().join("huge.md");
    // 2 MiB, comfortably over the 1 MB limit (size check truncates to whole MB).
    std::fs::write(&path, vec![b'a'; 2 * 1024 * 1024])?;

    let result = processor.process_file(&path).await?;

    let reason = skip_reason(result.status)?;
    assert!(
        reason.contains("too large") || reason.contains("MB"),
        "unexpected reason: {reason}"
    );
    assert!(result.conceptualization.is_none());
    assert!(result.clarity_improvement.is_none());
    Ok(())
}

#[tokio::test]
async fn file_under_the_limit_is_not_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let (processor, dir) = new_processor(10)?; // 10 MB limit
    let path = dir.path().join("small.md");
    std::fs::write(&path, "a small document")?;

    // Even though the AI endpoint is unreachable, conceptualize/clarity calls
    // for md/txt/rst/asciidoc files use `.ok()` internally, so failures are
    // swallowed and processing still completes rather than skipping.
    let result = processor.process_file(&path).await?;
    assert!(
        matches!(result.status, ProcessingStatus::Completed),
        "expected Completed, got {:?}",
        result.status
    );
    Ok(())
}

#[tokio::test]
async fn file_exactly_at_limit_boundary_is_not_skipped() -> Result<(), Box<dyn std::error::Error>> {
    // size_mb = len / (1024*1024) truncates, so a file of exactly 1 MiB
    // yields size_mb == 1 which is NOT > 1, so it must not be skipped.
    let (processor, dir) = new_processor(1)?;
    let path = dir.path().join("boundary.md");
    std::fs::write(&path, vec![b'a'; 1024 * 1024])?;

    let result = processor.process_file(&path).await?;
    assert!(matches!(result.status, ProcessingStatus::Completed));
    Ok(())
}

#[tokio::test]
async fn non_document_extension_propagates_ai_failure_as_error(
) -> Result<(), Box<dyn std::error::Error>> {
    // Files outside the txt/md/rst/asciidoc match arm go through
    // `improve_clarity_with_llama(..).await?` directly (no `.ok()`), so an
    // unreachable AI endpoint must surface as an `Err`, not be swallowed.
    let (processor, dir) = new_processor(10)?;
    let path = dir.path().join("legacy.doc");
    std::fs::write(&path, "legacy content")?;

    let result = processor.process_file(&path).await;
    assert!(result.is_err(), "expected AI failure to propagate as Err");
    Ok(())
}

// ---- debounce coalescing ------------------------------------------------

#[tokio::test]
async fn repeated_events_for_same_path_coalesce_into_one_pending_entry(
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut processor, dir) = new_processor(10)?;
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
    Ok(())
}

#[tokio::test]
async fn events_for_different_paths_stay_separate() -> Result<(), Box<dyn std::error::Error>> {
    let (mut processor, dir) = new_processor(10)?;
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
    Ok(())
}

#[tokio::test]
async fn deleted_event_removes_path_from_pending_queue() -> Result<(), Box<dyn std::error::Error>> {
    let (mut processor, dir) = new_processor(10)?;
    let path: PathBuf = dir.path().join("notes.md");
    let debounce = Duration::from_millis(2000);

    processor
        .handle_file_event(
            FileEvent {
                path: path.clone(),
                event_type: FileEventType::Created,
            },
            debounce,
        )
        .await;
    assert_eq!(processor.pending_count(), 1);

    processor
        .handle_file_event(
            FileEvent {
                path,
                event_type: FileEventType::Deleted,
            },
            debounce,
        )
        .await;
    assert_eq!(processor.pending_count(), 0);
    Ok(())
}
