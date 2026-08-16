//! CLI argument parsing, exercised against the real `goglz` binary (same
//! pattern as dreamseq's `tests/cli_test.rs`: spawn `CARGO_BIN_EXE_goglz`
//! and assert on exit status / stdout / stderr).
//!
//! Every invocation here sets `HOME` to an isolated temp directory so tests
//! never read or write the real developer machine's `~/.goglz` config (which
//! may exist and contain real, unrelated settings) or `~/.goglz_output`.
use std::process::Command;

fn goglz_cmd(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_goglz"));
    cmd.env("HOME", home);
    cmd
}

#[test]
fn help_lists_all_subcommands() {
    let home = tempfile::tempdir().unwrap();
    let output = goglz_cmd(home.path()).arg("--help").output().unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for command in ["start", "stop", "status", "init-config", "revise"] {
        assert!(help.contains(command), "missing `{command}` in --help output: {help}");
    }
}

#[test]
fn revise_help_lists_directory_flag() {
    let home = tempfile::tempdir().unwrap();
    let output = goglz_cmd(home.path())
        .args(["revise", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--directory"));
}

#[test]
fn unknown_subcommand_fails_cleanly_with_nonzero_exit() {
    let home = tempfile::tempdir().unwrap();
    let output = goglz_cmd(home.path()).arg("not-a-real-command").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.is_empty(), "clap should print a usage/error message to stderr");
}

#[test]
fn revise_rejects_unknown_flag() {
    let home = tempfile::tempdir().unwrap();
    let output = goglz_cmd(home.path())
        .args(["revise", "--not-a-real-flag"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn status_with_no_running_daemon_exits_cleanly() {
    let home = tempfile::tempdir().unwrap();
    let output = goglz_cmd(home.path()).arg("status").output().unwrap();
    assert!(output.status.success(), "status must exit 0 even with no daemon running");
}

#[test]
fn revise_directory_with_no_documents_processes_zero_and_touches_no_network() {
    // An empty target directory means discover_documents finds nothing, so
    // `run()` never calls the AI client at all - this must complete quickly
    // and successfully regardless of network availability or API keys.
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    let output = goglz_cmd(home.path())
        .args(["revise", "--directory"])
        .arg(target.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Total documents processed: 0"), "stdout: {stdout}");
}

#[test]
fn revise_directory_leaves_non_document_files_untouched() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let binary_path = target.path().join("data.bin");
    std::fs::write(&binary_path, [0u8, 159, 146, 150]).unwrap();

    let output = goglz_cmd(home.path())
        .args(["revise", "--directory"])
        .arg(target.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(std::fs::read(&binary_path).unwrap(), vec![0u8, 159, 146, 150]);
}
