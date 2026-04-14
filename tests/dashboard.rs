use assert_cmd::Command;
use github_actionspec_rs::types::{ActualValidationReport, ValidationReport, ValidationStatus};
use tempfile::tempdir;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

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
        matrix: Some(BTreeMap::from([(
            "app".to_string(),
            Value::String("build-ts-service".to_owned()),
        )])),
        outputs: Some(BTreeMap::from([(
            "build".to_string(),
            BTreeMap::from([
                (
                    "artifact_name".to_string(),
                    "build-ts-service-linux-amd64".to_string(),
                ),
                ("contract_build".to_string(), "build-ts-service".to_string()),
            ]),
        )])),
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
        .arg("--output-key")
        .arg("contract_build")
        .arg("--output")
        .arg(&output);

    command.assert().success();

    let markdown = std::fs::read_to_string(output).unwrap();
    assert!(markdown.contains("Validation Matrix"));
    assert!(markdown.contains("app=build-ts-service"));
    assert!(markdown.contains("build.contract_build=build-ts-service"));
    assert!(!markdown.contains("artifact_name"));
    assert!(markdown.contains("status Failed->Passed"));
    assert!(markdown.contains("build skipped->success"));
}
