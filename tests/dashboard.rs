use assert_cmd::Command;
use github_actionspec_rs::types::{ActualValidationReport, ValidationReport, ValidationStatus};
use tempfile::tempdir;

use std::collections::BTreeMap;
use std::path::PathBuf;

fn actual(path: &str, status: ValidationStatus, jobs: &[(&str, &str)]) -> ActualValidationReport {
    ActualValidationReport {
        actual_path: PathBuf::from(path),
        workflow: "ci.yml".to_owned(),
        ref_name: Some("main".to_owned()),
        status,
        jobs: jobs
            .iter()
            .map(|(name, result)| (name.to_string(), result.to_string()))
            .collect::<BTreeMap<_, _>>(),
        error: None,
    }
}

#[test]
fn dashboard_cli_writes_markdown_with_diff() {
    let temp = tempdir().unwrap();
    let current = temp.path().join("current.json");
    let baseline = temp.path().join("baseline.json");
    let output = temp.path().join("dashboard.md");

    std::fs::write(
        &current,
        serde_json::to_string_pretty(&ValidationReport {
            workflow: "ci.yml".to_owned(),
            declaration_path: PathBuf::from(".github/actionspec/ci/main.cue"),
            actuals: vec![actual(
                "tests/fixtures/ci/ci-main-pages.json",
                ValidationStatus::Passed,
                &[("build", "success"), ("pages", "success")],
            )],
        })
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        &baseline,
        serde_json::to_string_pretty(&ValidationReport {
            workflow: "ci.yml".to_owned(),
            declaration_path: PathBuf::from(".github/actionspec/ci/main.cue"),
            actuals: vec![actual(
                "tests/fixtures/ci/ci-main-pages.json",
                ValidationStatus::Failed,
                &[("build", "skipped"), ("pages", "skipped")],
            )],
        })
        .unwrap(),
    )
    .unwrap();

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .arg("dashboard")
        .arg("--current")
        .arg(&current)
        .arg("--baseline")
        .arg(&baseline)
        .arg("--output")
        .arg(&output);

    command.assert().success();

    let markdown = std::fs::read_to_string(output).unwrap();
    assert!(markdown.contains("Validation Matrix"));
    assert!(markdown.contains("status Failed->Passed"));
    assert!(markdown.contains("build skipped->success"));
}
