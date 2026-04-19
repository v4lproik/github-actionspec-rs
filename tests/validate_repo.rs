mod support;

use assert_cmd::Command;
use github_actionspec_rs::types::ValidationReport;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn validates_repo_contract_through_cli() {
    let repo = tempdir().unwrap();
    support::write_declaration(repo.path(), ".github/actionspec/ci/main.cue", "ci.yml");
    let actual = repo.path().join("actual.json");
    support::write_actual(&actual, "ci.yml");
    let env = support::install_fake_cue(&repo, "success");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--actual")
        .arg(&actual);

    command.assert().success();
}

#[test]
fn validates_repo_contract_directory_through_cli() {
    let repo = tempdir().unwrap();
    support::write_declaration(repo.path(), ".github/actionspec/ci/main.cue", "ci.yml");
    let actual_dir = repo.path().join("actuals");
    std::fs::create_dir_all(&actual_dir).unwrap();
    let actual_one = actual_dir.join("actual-one.json");
    let actual_two = actual_dir.join("actual-two.json");
    support::write_actual(&actual_one, "ci.yml");
    support::write_actual(&actual_two, "ci.yml");
    let env = support::install_fake_cue(&repo, "success");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--actual")
        .arg(&actual_dir);

    command.assert().success();
}

#[test]
fn infers_workflow_from_single_actual_through_cli() {
    let repo = tempdir().unwrap();
    support::write_declaration(repo.path(), ".github/actionspec/ci/main.cue", "ci.yml");
    let actual = repo.path().join("actual.json");
    support::write_actual(&actual, "ci.yml");
    let env = support::install_fake_cue(&repo, "success");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--actual")
        .arg(&actual);

    command.assert().success();
}

#[test]
fn errors_when_workflow_cannot_be_inferred_from_mixed_actuals() {
    let repo = tempdir().unwrap();
    let first = repo.path().join("first.json");
    let second = repo.path().join("second.json");
    support::write_actual(&first, "build.yml");
    support::write_actual(&second, "deploy.yml");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--actual")
        .arg(&first)
        .arg("--actual")
        .arg(&second);

    command
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Could not infer a single workflow from the provided actual payloads",
        ))
        .stderr(predicate::str::contains("build.yml"))
        .stderr(predicate::str::contains("deploy.yml"));
}

#[test]
fn validates_repo_contract_globbed_actuals_through_cli() {
    let repo = tempdir().unwrap();
    support::write_declaration(repo.path(), ".github/actionspec/ci/main.cue", "ci.yml");
    let actual_dir = repo.path().join("actuals");
    std::fs::create_dir_all(&actual_dir).unwrap();
    let actual_one = actual_dir.join("actual-one.json");
    let actual_two = actual_dir.join("actual-two.json");
    support::write_actual(&actual_one, "ci.yml");
    support::write_actual(&actual_two, "ci.yml");
    let env = support::install_fake_cue(&repo, "success");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--actual")
        .arg(actual_dir.join("*.json"));

    command.assert().success();
}

#[test]
fn discovers_repo_contracts_through_cli() {
    let repo = tempdir().unwrap();
    support::write_declaration(
        repo.path(),
        ".github/actionspec/release/default.cue",
        "release.yml",
    );

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command.arg("discover").arg("--repo").arg(repo.path());

    command
        .assert()
        .success()
        .stdout(predicate::str::contains("release.yml"))
        .stdout(predicate::str::contains(
            ".github/actionspec/release/default.cue",
        ));
}

#[test]
fn writes_report_file_before_failing_validation() {
    let repo = tempdir().unwrap();
    support::write_declaration(repo.path(), ".github/actionspec/ci/main.cue", "ci.yml");
    let passing = repo.path().join("ci-main-success.json");
    let failing = repo.path().join("ci-main-skipped.json");
    std::fs::write(
        &passing,
        "{\"run\":{\"workflow\":\"ci.yml\",\"ref\":\"main\",\"jobs\":{\"build\":{\"result\":\"success\"}}}}",
    )
    .unwrap();
    std::fs::write(
        &failing,
        "{\"run\":{\"workflow\":\"ci.yml\",\"ref\":\"main\",\"jobs\":{\"build\":{\"result\":\"skipped\"}}}}",
    )
    .unwrap();
    let report = repo.path().join("validation-report.json");
    let env = support::install_fake_cue_script(
        repo.path(),
        "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"vet\" ]; then\n  last=\"\"\n  for arg in \"$@\"; do\n    last=\"$arg\"\n  done\n  case \"$last\" in\n    *ci-main-success.json) exit 0 ;;\n    *ci-main-skipped.json) echo \"build should not be skipped\" >&2; exit 9 ;;\n  esac\nfi\nexit 1\n",
    );

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--actual")
        .arg(&passing)
        .arg("--actual")
        .arg(&failing)
        .arg("--report-file")
        .arg(&report);

    command
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Validation failed for 1 of 2 payloads.",
        ))
        .stderr(predicate::str::contains(failing.display().to_string()))
        .stderr(predicate::str::contains("build should not be skipped"));

    let report: ValidationReport =
        serde_json::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    assert_eq!(report.actuals.len(), 2);
    assert!(report.actuals.iter().any(|actual| {
        actual.actual_path == failing
            && actual
                .error
                .as_deref()
                .is_some_and(|error| error.contains("cue vet failed for"))
            && actual
                .error
                .as_deref()
                .is_some_and(|error| error.contains("build should not be skipped"))
            && actual.issues.len() == 1
            && actual.issues[0]
                .message
                .contains("build should not be skipped")
    }));
}

#[test]
fn dry_run_preserves_validation_report_without_failing() {
    let repo = tempdir().unwrap();
    support::write_declaration(repo.path(), ".github/actionspec/ci/main.cue", "ci.yml");
    let failing = repo.path().join("ci-main-skipped.json");
    std::fs::write(
        &failing,
        "{\"run\":{\"workflow\":\"ci.yml\",\"ref\":\"main\",\"jobs\":{\"build\":{\"result\":\"skipped\"}}}}",
    )
    .unwrap();
    let report = repo.path().join("validation-report.json");
    let env = support::install_fake_cue_script(
        repo.path(),
        "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"vet\" ]; then\n  echo \"build should not be skipped\" >&2\n  exit 9\nfi\nexit 1\n",
    );

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--actual")
        .arg(&failing)
        .arg("--report-file")
        .arg(&report)
        .arg("--dry-run");

    command.assert().success();

    let report: ValidationReport =
        serde_json::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    assert_eq!(report.actuals.len(), 1);
    assert_eq!(
        report.actuals[0].status,
        github_actionspec_rs::types::ValidationStatus::Failed
    );
    assert!(report.actuals[0]
        .error
        .as_deref()
        .is_some_and(|error| error.contains("build should not be skipped")));
    assert_eq!(report.actuals[0].issues.len(), 1);
    assert!(report.actuals[0].issues[0]
        .message
        .contains("build should not be skipped"));
}

#[test]
fn dry_run_creates_missing_parent_directories_for_validation_report() {
    let repo = tempdir().unwrap();
    support::write_declaration(repo.path(), ".github/actionspec/ci/main.cue", "ci.yml");
    let failing = repo.path().join("ci-main-skipped.json");
    std::fs::write(
        &failing,
        "{\"run\":{\"workflow\":\"ci.yml\",\"ref\":\"main\",\"jobs\":{\"build\":{\"result\":\"skipped\"}}}}",
    )
    .unwrap();
    let report = repo
        .path()
        .join("target")
        .join("actionspec")
        .join("validation-report.json");
    let env = support::install_fake_cue_script(
        repo.path(),
        "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"vet\" ]; then\n  echo \"build should not be skipped\" >&2\n  exit 9\nfi\nexit 1\n",
    );

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--actual")
        .arg(&failing)
        .arg("--report-file")
        .arg(&report)
        .arg("--dry-run");

    command.assert().success();

    assert!(
        report.is_file(),
        "report should be created in nested directories"
    );
}

#[test]
fn report_file_preserves_matrix_metadata() {
    let repo = tempdir().unwrap();
    support::write_declaration(
        repo.path(),
        ".github/actionspec/build/main.cue",
        "build.yml",
    );
    let actual = repo.path().join("build-ts-service.json");
    std::fs::write(
        &actual,
        r#"{"run":{"workflow":"build.yml","jobs":{"build":{"result":"success","matrix":{"app":"build-ts-service","target":"linux-amd64"},"outputs":{"contract_build":"build-ts-service","artifact_name":"build-ts-service-linux-amd64"}}}}}"#,
    )
    .unwrap();
    let report = repo.path().join("validation-report.json");
    let env = support::install_fake_cue(&repo, "success");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("build.yml")
        .arg("--actual")
        .arg(&actual)
        .arg("--report-file")
        .arg(&report);

    command.assert().success();

    let report: ValidationReport =
        serde_json::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    assert_eq!(report.actuals.len(), 1);
    assert_eq!(
        report.actuals[0].matrix,
        Some(std::collections::BTreeMap::from([
            (
                "app".to_string(),
                Value::String("build-ts-service".to_owned()),
            ),
            (
                "target".to_string(),
                Value::String("linux-amd64".to_owned()),
            ),
        ]))
    );
    assert_eq!(
        report.actuals[0].outputs,
        Some(std::collections::BTreeMap::from([(
            "build".to_string(),
            std::collections::BTreeMap::from([
                (
                    "artifact_name".to_string(),
                    "build-ts-service-linux-amd64".to_string(),
                ),
                ("contract_build".to_string(), "build-ts-service".to_string(),),
            ]),
        )]))
    );
}
