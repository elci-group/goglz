// SPDX-License-Identifier: MIT
//! Portfolio-mode integration tests.
mod support;

use std::fs;

fn goglz_cmd(home: &std::path::Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_goglz"));
    cmd.env("HOME", home);
    cmd
}

#[test]
fn help_lists_portfolio_flag() -> Result<(), Box<dyn std::error::Error>> {
    let home = tempfile::tempdir()?;
    let output = goglz_cmd(home.path()).arg("--help").output()?;
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("--portfolio"),
        "missing `--portfolio` in --help output: {help}"
    );
    Ok(())
}

#[test]
fn revise_portfolio_processes_each_project_and_skips_non_projects(
) -> Result<(), Box<dyn std::error::Error>> {
    // HOME layout:
    //   ~/project-alpha/goglz.yaml
    //   ~/project-beta/goglz.yaml
    //   ~/not-a-project/readme.md   <-- should be ignored (no goglz.yaml)
    //
    // Projects are kept empty so the revise pipeline never calls the AI and
    // the test stays deterministic without network access or API keys.
    let home = tempfile::tempdir()?;

    fs::create_dir_all(home.path().join("project-alpha"))?;
    fs::create_dir_all(home.path().join("project-beta"))?;
    fs::create_dir_all(home.path().join("not-a-project"))?;

    let goglz_yaml = r#"purpose: Improve document clarity
scope: All documentation files
writing_style:
  tone: Professional
  voice: Objective
  audience: General
  guidelines:
    - Be concise
formatting_rules:
  headings: true
  bullet_points: true
  numbered_lists: true
  code_blocks: true
  max_line_length: 80
  custom_rules: []
global_assets: []
local_assets: []
languages: []
"#;

    fs::write(home.path().join("project-alpha/goglz.yaml"), goglz_yaml)?;
    fs::write(home.path().join("project-beta/goglz.yaml"), goglz_yaml)?;
    fs::write(home.path().join("not-a-project/readme.md"), "# Orphan")?;

    let output = goglz_cmd(home.path())
        .args(["--portfolio", "revise"])
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("project-alpha"), "stdout: {stdout}");
    assert!(stdout.contains("project-beta"), "stdout: {stdout}");
    assert!(
        !stdout.contains("not-a-project"),
        "non-project directory should not be processed: {stdout}"
    );

    // Each empty project should report zero processed documents.
    assert!(
        stdout.contains("Total documents processed: 0"),
        "expected per-project count in stdout: {stdout}"
    );
    Ok(())
}

#[test]
fn revise_portfolio_with_no_projects_warns_and_exits_cleanly(
) -> Result<(), Box<dyn std::error::Error>> {
    let home = tempfile::tempdir()?;

    let output = goglz_cmd(home.path())
        .args(["--portfolio", "revise"])
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("no goglz projects found"),
        "stdout: {stdout}"
    );
    Ok(())
}

#[test]
fn portfolio_flag_is_ignored_for_status() -> Result<(), Box<dyn std::error::Error>> {
    let home = tempfile::tempdir()?;
    let output = goglz_cmd(home.path())
        .args(["--portfolio", "status"])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--portfolio has no effect"),
        "stdout: {stdout}"
    );
    Ok(())
}
