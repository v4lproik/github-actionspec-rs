use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
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
