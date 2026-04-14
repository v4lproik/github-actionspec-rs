use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

fn write_workflow(repo_root: &Path, relative_path: &str, contents: &str) {
    let path = repo_root.join(relative_path);
    let parent = path.parent().expect("workflow file should have a parent");
    fs::create_dir_all(parent).expect("workflow directory should be created");
    fs::write(path, contents).expect("workflow fixture should be written");
}

#[test]
fn validates_local_reusable_workflow_callers_through_cli() {
    let repo = tempdir().expect("temp dir should be created");

    write_workflow(
        repo.path(),
        ".github/workflows/reusable-build.yml",
        r#"on:
  workflow_call:
    inputs:
      environment:
        type: string
        required: true
    outputs:
      image_tag:
        value: ${{ jobs.build.outputs.image_tag }}
jobs:
  build:
    runs-on: ubuntu-latest
"#,
    );
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  push:
jobs:
  build:
    uses: ./.github/workflows/reusable-build.yml
    with:
      environment: staging
  summarize:
    runs-on: ubuntu-latest
    needs: [build]
    steps:
      - run: echo "${{ needs.build.outputs.image_tag }}"
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("validate-callers")
        .arg("--repo")
        .arg(repo.path());

    command.assert().success();
}

#[test]
fn reports_reusable_workflow_interface_failures_through_cli() {
    let repo = tempdir().expect("temp dir should be created");

    write_workflow(
        repo.path(),
        ".github/workflows/reusable-build.yml",
        r#"on:
  workflow_call:
    inputs:
      changed:
        type: boolean
        required: true
    outputs:
      image_tag:
        value: ${{ jobs.build.outputs.image_tag }}
jobs:
  build:
    runs-on: ubuntu-latest
"#,
    );
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  pull_request:
jobs:
  build:
    uses: ./.github/workflows/reusable-build.yml
    with:
      changed: maybe
      extra-input: true
  summarize:
    runs-on: ubuntu-latest
    needs: [build]
    steps:
      - run: echo "${{ needs.build.outputs.missing_tag }}"
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("validate-callers")
        .arg("--repo")
        .arg(repo.path());

    command
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Reusable workflow validation failed",
        ))
        .stderr(predicate::str::contains(
            "input `changed` expects a boolean value",
        ))
        .stderr(predicate::str::contains("unexpected input `extra-input`"))
        .stderr(predicate::str::contains(
            "missing reusable workflow output `missing_tag`",
        ));
}

#[test]
fn dry_run_writes_caller_report_without_failing() {
    let repo = tempdir().expect("temp dir should be created");
    let report = repo.path().join("callers-report.json");

    write_workflow(
        repo.path(),
        ".github/workflows/reusable-build.yml",
        r#"on:
  workflow_call:
    inputs:
      changed:
        type: boolean
        required: true
    outputs:
      image_tag:
        value: ${{ jobs.build.outputs.image_tag }}
jobs:
  build:
    runs-on: ubuntu-latest
"#,
    );
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  pull_request:
jobs:
  build:
    uses: ./.github/workflows/reusable-build.yml
    with:
      changed: maybe
  summarize:
    runs-on: ubuntu-latest
    needs: [build]
    steps:
      - run: echo "${{ needs.build.outputs.missing_tag }}"
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("validate-callers")
        .arg("--repo")
        .arg(repo.path())
        .arg("--report-file")
        .arg(&report)
        .arg("--dry-run");

    command.assert().success();

    let report: Value =
        serde_json::from_str(&fs::read_to_string(report).expect("report should be readable"))
            .expect("report should be valid json");
    assert_eq!(report["issues"].as_array().map(Vec::len), Some(2));
    assert_eq!(report["workflows"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        report["workflows"][0]["calls"][0]["referenced_outputs"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn dry_run_creates_missing_parent_directories_for_caller_report() {
    let repo = tempdir().expect("temp dir should be created");
    let report = repo
        .path()
        .join("target")
        .join("actionspec")
        .join("callers-report.json");

    write_workflow(
        repo.path(),
        ".github/workflows/reusable-build.yml",
        r#"on:
  workflow_call:
    inputs:
      changed:
        type: boolean
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
"#,
    );
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  pull_request:
jobs:
  build:
    uses: ./.github/workflows/reusable-build.yml
    with:
      changed: maybe
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("validate-callers")
        .arg("--repo")
        .arg(repo.path())
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
fn dry_run_report_tracks_issues_per_reusable_workflow_call() {
    let repo = tempdir().expect("temp dir should be created");
    let report = repo.path().join("callers-report.json");

    write_workflow(
        repo.path(),
        ".github/workflows/reusable-build.yml",
        r#"on:
  workflow_call:
    inputs:
      environment:
        type: string
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
"#,
    );
    write_workflow(
        repo.path(),
        ".github/workflows/reusable-publish.yml",
        r#"on:
  workflow_call:
    outputs:
      image_tag:
        value: ${{ jobs.publish.outputs.image_tag }}
jobs:
  publish:
    runs-on: ubuntu-latest
"#,
    );
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  pull_request:
jobs:
  build:
    uses: ./.github/workflows/reusable-build.yml
  publish:
    uses: ./.github/workflows/reusable-publish.yml
  summarize:
    runs-on: ubuntu-latest
    needs: [publish]
    steps:
      - run: echo "${{ needs.publish.outputs.missing_tag }}"
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("validate-callers")
        .arg("--repo")
        .arg(repo.path())
        .arg("--report-file")
        .arg(&report)
        .arg("--dry-run");

    command.assert().success();

    let report: Value =
        serde_json::from_str(&fs::read_to_string(report).expect("report should be readable"))
            .expect("report should be valid json");
    assert_eq!(report["issues"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        report["workflows"][0]["calls"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        report["workflows"][0]["calls"][0]["job_id"].as_str(),
        Some("build")
    );
    assert_eq!(
        report["workflows"][0]["calls"][0]["issues"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        report["workflows"][0]["calls"][1]["job_id"].as_str(),
        Some("publish")
    );
    assert_eq!(
        report["workflows"][0]["calls"][1]["referenced_outputs"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        report["workflows"][0]["calls"][1]["issues"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}
