use goglz::planning::{check, generate, revise, PlanOptions, ARTIFACTS};
use std::fs;

#[test]
fn generation_creates_the_complete_strategy_set_and_is_non_destructive() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        generate(dir.path(), Some("Acme"), false).unwrap(),
        ARTIFACTS.len()
    );
    assert_eq!(generate(dir.path(), Some("Changed"), false).unwrap(), 0);
    assert!(dir.path().join(".goglz/plan/database.md").exists());
}

#[test]
fn checks_require_padagonia_schema_and_growth_controls() {
    let dir = tempfile::tempdir().unwrap();
    generate(dir.path(), None, false).unwrap();
    let report = check(dir.path()).unwrap();
    assert!(
        report.passed,
        "failed checks: {:?}",
        report
            .results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| &r.name)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn local_revision_preserves_content_and_adds_the_brief_without_network() {
    let dir = tempfile::tempdir().unwrap();
    generate(dir.path(), None, false).unwrap();
    let before = fs::read_to_string(dir.path().join(".goglz/plan/prd.md")).unwrap();
    let options = PlanOptions {
        artifact: Some("prd".into()),
        instruction: "Add a discovery owner".into(),
        ai: false,
    };
    assert_eq!(
        revise(dir.path(), &options, || unreachable!())
            .await
            .unwrap(),
        1
    );
    let after = fs::read_to_string(dir.path().join(".goglz/plan/prd.md")).unwrap();
    assert!(after.starts_with(&before));
    assert!(after.contains("Add a discovery owner"));
}
