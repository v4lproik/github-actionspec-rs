use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn write_workflow(repo_root: &std::path::Path, relative_path: &str, contents: &str) {
    let path = repo_root.join(relative_path);
    fs::create_dir_all(path.parent().expect("workflow parent should exist"))
        .expect("workflow dir should be created");
    fs::write(path, contents).expect("workflow should be written");
}

#[test]
fn bootstrap_cli_generates_starter_contract_and_baseline_from_workflow() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  pull_request:
jobs:
  lint:
    runs-on: ubuntu-latest
  build:
    runs-on: ubuntu-latest
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml");

    command
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Created starter workflow contract artifacts",
        ))
        .stdout(predicate::str::contains(".github/actionspec/ci/main.cue"))
        .stdout(predicate::str::contains("tests/fixtures/ci/baseline.json"));

    let declaration = repo.path().join(".github/actionspec/ci/main.cue");
    let baseline = repo.path().join("tests/fixtures/ci/baseline.json");
    let snippet = repo
        .path()
        .join(".github/actionspec/ci/bootstrap-ci-snippet.yml");

    let declaration_contents =
        fs::read_to_string(&declaration).expect("declaration should be readable");
    let baseline_contents = fs::read_to_string(&baseline).expect("baseline should be readable");
    let snippet_contents = fs::read_to_string(&snippet).expect("snippet should be readable");

    assert!(declaration_contents.contains("workflow: \"ci.yml\""));
    assert!(declaration_contents.contains("\"build\": {"));
    assert!(declaration_contents.contains("\"lint\": {"));
    assert!(baseline_contents.contains("\"workflow\": \"ci.yml\""));
    assert!(baseline_contents.contains("\"result\": \"success\""));
    assert!(snippet_contents.contains("Validate ci.yml contract"));
    assert!(snippet_contents.contains("uses: v4lproik/github-actionspec-rs@main"));
    assert!(snippet_contents.contains("actual: tests/fixtures/ci/baseline.json"));
}

#[test]
fn bootstrap_cli_seeds_from_existing_actual_payload() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".github/workflows/build.yml",
        r#"on:
  push:
jobs:
  build:
    runs-on: ubuntu-latest
"#,
    );
    let actual = repo.path().join("captured.json");
    fs::write(
        &actual,
        r#"{
  "run": {
    "workflow": "build.yml",
    "ref": "main",
    "jobs": {
      "build": {
        "result": "success",
        "outputs": {
          "artifact_name": "build-linux-amd64"
        },
        "matrix": {
          "app": "build-ts-service"
        },
        "steps": {
          "compile": {
            "conclusion": "success"
          }
        }
      }
    }
  }
}"#,
    )
    .expect("actual payload should be written");

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("build.yml")
        .arg("--actual")
        .arg(&actual);

    command
        .assert()
        .success()
        .stdout(predicate::str::contains("seeded from actual: yes"));

    let declaration = repo.path().join(".github/actionspec/build/main.cue");
    let baseline = repo.path().join("tests/fixtures/build/baseline.json");
    let snippet = repo
        .path()
        .join(".github/actionspec/build/bootstrap-ci-snippet.yml");

    let declaration_contents =
        fs::read_to_string(&declaration).expect("declaration should be readable");
    let baseline_contents = fs::read_to_string(&baseline).expect("baseline should be readable");
    let snippet_contents = fs::read_to_string(&snippet).expect("snippet should be readable");

    assert!(declaration_contents.contains("ref: \"main\""));
    assert!(declaration_contents.contains("\"artifact_name\": \"build-linux-amd64\""));
    assert!(declaration_contents.contains("\"app\": \"build-ts-service\""));
    assert!(declaration_contents.contains("\"compile\": {"));
    assert!(baseline_contents.contains("\"artifact_name\": \"build-linux-amd64\""));
    assert!(snippet_contents.contains("name: actionspec-build-baseline"));
}

#[test]
fn bootstrap_cli_requires_force_to_overwrite_existing_outputs() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  push:
jobs:
  build:
    runs-on: ubuntu-latest
"#,
    );
    let declaration = repo.path().join(".github/actionspec/ci/main.cue");
    fs::create_dir_all(
        declaration
            .parent()
            .expect("declaration parent should exist"),
    )
    .expect("declaration dir should be created");
    fs::write(&declaration, "existing").expect("existing declaration should be written");

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml");

    command.assert().failure().stderr(predicate::str::contains(
        "Bootstrap would overwrite an existing file",
    ));
}

#[test]
fn bootstrap_cli_overwrites_existing_outputs_when_force_is_set() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  push:
jobs:
  lint:
    runs-on: ubuntu-latest
"#,
    );
    let declaration = repo.path().join(".github/actionspec/ci/main.cue");
    let baseline = repo.path().join("tests/fixtures/ci/baseline.json");
    let snippet = repo
        .path()
        .join(".github/actionspec/ci/bootstrap-ci-snippet.yml");
    fs::create_dir_all(
        declaration
            .parent()
            .expect("declaration parent should exist"),
    )
    .expect("declaration dir should be created");
    fs::create_dir_all(baseline.parent().expect("baseline parent should exist"))
        .expect("baseline dir should be created");
    fs::write(&declaration, "stale contract").expect("existing declaration should be written");
    fs::write(&baseline, "{\"stale\":true}").expect("existing baseline should be written");
    fs::write(&snippet, "stale snippet").expect("existing snippet should be written");

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--force");

    command.assert().success();

    let declaration_contents =
        fs::read_to_string(&declaration).expect("declaration should be readable");
    let baseline_contents = fs::read_to_string(&baseline).expect("baseline should be readable");
    let snippet_contents = fs::read_to_string(&snippet).expect("snippet should be readable");

    assert!(declaration_contents.contains("workflow: \"ci.yml\""));
    assert!(!declaration_contents.contains("stale contract"));
    assert!(baseline_contents.contains("\"workflow\": \"ci.yml\""));
    assert!(!baseline_contents.contains("\"stale\":true"));
    assert!(snippet_contents.contains("Upload validation artifacts"));
    assert!(!snippet_contents.contains("stale snippet"));
}

#[test]
fn bootstrap_cli_rejects_actual_payloads_for_a_different_workflow() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  push:
jobs:
  build:
    runs-on: ubuntu-latest
"#,
    );
    let actual = repo.path().join("captured.json");
    fs::write(
        &actual,
        r#"{
  "run": {
    "workflow": "deploy.yml",
    "jobs": {
      "build": {
        "result": "success"
      }
    }
  }
}"#,
    )
    .expect("actual payload should be written");

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--actual")
        .arg(&actual);

    command
        .assert()
        .failure()
        .stderr(predicate::str::contains("belongs to workflow `deploy.yml`"))
        .stderr(predicate::str::contains("expected `ci.yml`"));
}

#[test]
fn bootstrap_cli_rejects_workflows_without_jobs() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".github/workflows/ci.yml",
        r#"on:
  push:
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("ci.yml");

    command
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not define any jobs"))
        .stderr(predicate::str::contains(".github/workflows/ci.yml"));
}

#[test]
fn bootstrap_cli_handles_complex_monorepo_style_workflow_from_yaml_only() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".github/workflows/build-infrastructure.yml",
        r#"on:
  pull_request:
    paths:
      - services/**
      - .github/workflows/build-infrastructure.yml
  workflow_dispatch:
jobs:
  detect-changes:
    runs-on: ubuntu-latest
    outputs:
      run_build: ${{ steps.filter.outputs.run_build }}
      run_publish: ${{ steps.filter.outputs.run_publish }}
    steps:
      - id: filter
        run: echo "filter"
  lint:
    needs: detect-changes
    if: needs.detect-changes.outputs.run_build == 'true'
    runs-on: ubuntu-latest
  build:
    needs:
      - detect-changes
      - lint
    if: needs.detect-changes.outputs.run_build == 'true'
    strategy:
      matrix:
        app:
          - api
          - worker
        target:
          - linux-amd64
          - linux-arm64
    uses: ./.github/workflows/reusable-build.yml
    with:
      app: ${{ matrix.app }}
      target: ${{ matrix.target }}
  publish:
    needs:
      - detect-changes
      - build
    if: needs.detect-changes.outputs.run_publish == 'true'
    runs-on: ubuntu-latest
    steps:
      - run: echo publish
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("build-infrastructure.yml");

    command.assert().success();

    let declaration = repo
        .path()
        .join(".github/actionspec/build-infrastructure/main.cue");
    let baseline = repo
        .path()
        .join("tests/fixtures/build-infrastructure/baseline.json");
    let snippet = repo
        .path()
        .join(".github/actionspec/build-infrastructure/bootstrap-ci-snippet.yml");

    let declaration_contents =
        fs::read_to_string(&declaration).expect("declaration should be readable");
    let baseline_contents = fs::read_to_string(&baseline).expect("baseline should be readable");
    let snippet_contents = fs::read_to_string(&snippet).expect("snippet should be readable");

    assert!(declaration_contents.contains("\"detect-changes\": {"));
    assert!(declaration_contents.contains("\"lint\": {"));
    assert!(declaration_contents.contains("\"build\": {"));
    assert!(declaration_contents.contains("\"publish\": {"));
    assert!(baseline_contents.contains("\"detect-changes\""));
    assert!(baseline_contents.contains("\"publish\""));
    assert!(snippet_contents.contains("workflow: build-infrastructure.yml"));
    assert!(snippet_contents.contains("actual: tests/fixtures/build-infrastructure/baseline.json"));
}

#[test]
fn bootstrap_cli_seeds_complex_actual_payload_with_inputs_outputs_and_multiple_jobs() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".github/workflows/release.yml",
        r#"on:
  workflow_dispatch:
    inputs:
      environment:
        required: true
jobs:
  prepare:
    runs-on: ubuntu-latest
  build:
    needs: prepare
    runs-on: ubuntu-latest
  deploy:
    needs: build
    runs-on: ubuntu-latest
"#,
    );
    let actual = repo.path().join("release-captured.json");
    fs::write(
        &actual,
        r#"{
  "run": {
    "workflow": "release.yml",
    "ref": "main",
    "inputs": {
      "environment": "staging",
      "promote": null
    },
    "jobs": {
      "prepare": {
        "result": "success",
        "outputs": {
          "release_id": "rel-123"
        },
        "steps": {
          "resolve": {
            "conclusion": "success",
            "outputs": {
              "channel": "beta"
            }
          }
        }
      },
      "build": {
        "result": "success",
        "matrix": {
          "app": "api",
          "shard": 2,
          "canary": true,
          "tenant": null
        },
        "outputs": {
          "image": "ghcr.io/example/api:sha-123"
        }
      },
      "deploy": {
        "result": "success",
        "outputs": {
          "environment_url": "https://staging.example.com"
        }
      }
    }
  }
}"#,
    )
    .expect("actual payload should be written");

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("release.yml")
        .arg("--actual")
        .arg(&actual);

    command.assert().success();

    let declaration = repo.path().join(".github/actionspec/release/main.cue");
    let snippet = repo
        .path()
        .join(".github/actionspec/release/bootstrap-ci-snippet.yml");
    let declaration_contents =
        fs::read_to_string(&declaration).expect("declaration should be readable");
    let snippet_contents = fs::read_to_string(&snippet).expect("snippet should be readable");

    assert!(declaration_contents.contains("ref: \"main\""));
    assert!(declaration_contents.contains("\"environment\": \"staging\""));
    assert!(declaration_contents.contains("\"promote\": null"));
    assert!(declaration_contents.contains("\"release_id\": \"rel-123\""));
    assert!(declaration_contents.contains("\"app\": \"api\""));
    assert!(declaration_contents.contains("\"shard\": 2"));
    assert!(declaration_contents.contains("\"canary\": true"));
    assert!(declaration_contents.contains("\"tenant\": null"));
    assert!(declaration_contents.contains("\"environment_url\": \"https://staging.example.com\""));
    assert!(snippet_contents.contains("name: actionspec-release-baseline"));
}

#[test]
fn bootstrap_cli_normalizes_workflow_paths_and_supports_custom_layouts() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".ci/workflows/release.yml",
        r#"on:
  workflow_dispatch:
jobs:
  plan:
    runs-on: ubuntu-latest
  deploy:
    needs: plan
    runs-on: ubuntu-latest
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg(".ci/workflows/release.yml")
        .arg("--workflows-dir")
        .arg(".ci/workflows")
        .arg("--declarations-dir")
        .arg(".contracts/actionspec")
        .arg("--fixtures-dir")
        .arg(".contracts/fixtures");

    command
        .assert()
        .success()
        .stdout(predicate::str::contains("- workflow: release.yml"));

    let declaration = repo.path().join(".contracts/actionspec/release/main.cue");
    let baseline = repo
        .path()
        .join(".contracts/fixtures/release/baseline.json");
    let snippet = repo
        .path()
        .join(".contracts/actionspec/release/bootstrap-ci-snippet.yml");

    let declaration_contents =
        fs::read_to_string(&declaration).expect("declaration should be readable");
    let baseline_contents = fs::read_to_string(&baseline).expect("baseline should be readable");
    let snippet_contents = fs::read_to_string(&snippet).expect("snippet should be readable");

    assert!(declaration_contents.contains("workflow: \"release.yml\""));
    assert!(baseline_contents.contains("\"workflow\": \"release.yml\""));
    assert!(snippet_contents.contains("workflow: release.yml"));
    assert!(snippet_contents.contains("actual: .contracts/fixtures/release/baseline.json"));
    assert!(snippet_contents.contains("name: actionspec-release-baseline"));
}

#[test]
fn bootstrap_cli_reports_repo_relative_workflow_file_in_the_generated_summary() {
    let repo = tempdir().expect("temp dir should be created");
    write_workflow(
        repo.path(),
        ".github/workflows/deploy.yml",
        r#"on:
  push:
jobs:
  deploy:
    runs-on: ubuntu-latest
"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("bootstrap")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg(".github/workflows/deploy.yml");

    command
        .assert()
        .success()
        .stdout(predicate::str::contains("- workflow: deploy.yml"))
        .stdout(predicate::str::contains(".github/workflows/deploy.yml"));
}
