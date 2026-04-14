use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

fn write_fragment(temp_root: &Path, relative_path: &str, contents: &str) {
    let path = temp_root.join(relative_path);
    let parent = path
        .parent()
        .expect("fragment should have a parent directory");
    fs::create_dir_all(parent).expect("fragment directory should be created");
    fs::write(path, contents).expect("fragment should be written");
}

#[test]
fn capture_cli_writes_normalized_payload_from_job_fragments() {
    let temp = tempdir().expect("temp dir should be created");
    let output = temp
        .path()
        .join("target")
        .join("actionspec")
        .join("ci-main.json");

    write_fragment(
        temp.path(),
        "fragments/build.json",
        r#"{
  "job": "build",
  "result": "success",
  "outputs": {
    "artifact_name": "build-ts-service-linux-amd64"
  },
  "matrix": {
    "app": "build-ts-service",
    "target": "linux-amd64"
  },
  "steps": {
    "compile": {
      "conclusion": "success",
      "outputs": {
        "digest": "sha256:abc123"
      }
    }
  }
}"#,
    );
    write_fragment(
        temp.path(),
        "fragments/tests.json",
        r#"{
  "job": "tests",
  "result": "success"
}"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("capture")
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--ref")
        .arg("main")
        .arg("--input")
        .arg("run_ci=true")
        .arg("--job-file")
        .arg(temp.path().join("fragments"))
        .arg("--output")
        .arg(&output);

    command.assert().success();

    let payload: Value =
        serde_json::from_str(&fs::read_to_string(output).expect("output should be readable"))
            .expect("payload should be valid json");
    assert_eq!(payload["run"]["workflow"].as_str(), Some("ci.yml"));
    assert_eq!(payload["run"]["ref"].as_str(), Some("main"));
    assert_eq!(payload["run"]["inputs"]["run_ci"].as_str(), Some("true"));
    assert_eq!(
        payload["run"]["jobs"]["build"]["matrix"]["app"].as_str(),
        Some("build-ts-service")
    );
    assert_eq!(
        payload["run"]["jobs"]["build"]["steps"]["compile"]["outputs"]["digest"].as_str(),
        Some("sha256:abc123")
    );
    assert!(payload["run"]["jobs"]["tests"].get("outputs").is_none());
}

#[test]
fn capture_cli_reports_duplicate_job_fragments() {
    let temp = tempdir().expect("temp dir should be created");
    let first = temp.path().join("build-a.json");
    let second = temp.path().join("build-b.json");
    write_fragment(
        temp.path(),
        "build-a.json",
        r#"{"job":"build","result":"success"}"#,
    );
    write_fragment(
        temp.path(),
        "build-b.json",
        r#"{"job":"build","result":"failure"}"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("capture")
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--job-file")
        .arg(&first)
        .arg("--job-file")
        .arg(&second)
        .arg("--output")
        .arg(temp.path().join("actual.json"));

    command
        .assert()
        .failure()
        .stderr(predicate::str::contains("Duplicate captured job `build`"));
}
