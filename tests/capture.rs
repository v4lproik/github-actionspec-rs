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
fn emit_fragment_cli_writes_expected_job_fragment() {
    let temp = tempdir().expect("temp dir should be created");
    let output = temp.path().join("fragments").join("build.json");

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("emit-fragment")
        .arg("--job")
        .arg("build")
        .arg("--result")
        .arg("success")
        .arg("--output")
        .arg("contract_build=build-ts-service")
        .arg("--output")
        .arg("empty=")
        .arg("--matrix")
        .arg("app=build-ts-service")
        .arg("--matrix")
        .arg("shard=2")
        .arg("--step-conclusion")
        .arg("compile=success")
        .arg("--step-output")
        .arg("compile.digest=sha256:abc123")
        .arg("--file")
        .arg(&output);

    command.assert().success();

    let fragment: Value =
        serde_json::from_str(&fs::read_to_string(output).expect("output should be readable"))
            .expect("fragment should be valid json");
    assert_eq!(fragment["job"].as_str(), Some("build"));
    assert_eq!(fragment["result"].as_str(), Some("success"));
    assert_eq!(
        fragment["outputs"]["contract_build"].as_str(),
        Some("build-ts-service")
    );
    assert_eq!(fragment["outputs"]["empty"].as_str(), Some(""));
    assert_eq!(fragment["matrix"]["app"].as_str(), Some("build-ts-service"));
    assert_eq!(fragment["matrix"]["shard"].as_i64(), Some(2));
    assert_eq!(
        fragment["steps"]["compile"]["outputs"]["digest"].as_str(),
        Some("sha256:abc123")
    );
}

#[test]
fn emit_fragment_cli_rejects_duplicate_outputs() {
    let temp = tempdir().expect("temp dir should be created");

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("emit-fragment")
        .arg("--job")
        .arg("build")
        .arg("--result")
        .arg("success")
        .arg("--output")
        .arg("contract_build=one")
        .arg("--output")
        .arg("contract_build=two")
        .arg("--file")
        .arg(temp.path().join("build.json"));

    command.assert().failure().stderr(predicate::str::contains(
        "Duplicate emit-fragment output `contract_build`",
    ));
}

#[test]
fn emit_fragment_output_round_trips_through_capture() {
    let temp = tempdir().expect("temp dir should be created");
    let fragments_dir = temp.path().join("fragments");
    let actual = temp.path().join("actual.json");

    let mut emit_build = Command::cargo_bin("github-actionspec").expect("binary should exist");
    emit_build
        .arg("emit-fragment")
        .arg("--job")
        .arg("build")
        .arg("--result")
        .arg("success")
        .arg("--output")
        .arg("contract_build=build-ts-service")
        .arg("--matrix")
        .arg("app=build-ts-service")
        .arg("--file")
        .arg(fragments_dir.join("build.json"));
    emit_build.assert().success();

    let mut emit_publish = Command::cargo_bin("github-actionspec").expect("binary should exist");
    emit_publish
        .arg("emit-fragment")
        .arg("--job")
        .arg("publish")
        .arg("--result")
        .arg("success")
        .arg("--output")
        .arg("published_tag=ghcr.io/acme/app:sha-123")
        .arg("--file")
        .arg(fragments_dir.join("publish.json"));
    emit_publish.assert().success();

    let mut capture = Command::cargo_bin("github-actionspec").expect("binary should exist");
    capture
        .arg("capture")
        .arg("--workflow")
        .arg("release.yml")
        .arg("--job-file")
        .arg(&fragments_dir)
        .arg("--output")
        .arg(&actual);
    capture.assert().success();

    let payload: Value =
        serde_json::from_str(&fs::read_to_string(actual).expect("payload should be readable"))
            .expect("payload should be valid json");
    assert_eq!(payload["run"]["workflow"].as_str(), Some("release.yml"));
    assert_eq!(
        payload["run"]["jobs"]["build"]["outputs"]["contract_build"].as_str(),
        Some("build-ts-service")
    );
    assert_eq!(
        payload["run"]["jobs"]["build"]["matrix"]["app"].as_str(),
        Some("build-ts-service")
    );
    assert_eq!(
        payload["run"]["jobs"]["publish"]["outputs"]["published_tag"].as_str(),
        Some("ghcr.io/acme/app:sha-123")
    );
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

#[test]
fn capture_cli_accepts_globbed_job_fragments() {
    let temp = tempdir().expect("temp dir should be created");
    let output = temp.path().join("actual.json");

    write_fragment(
        temp.path(),
        "fragments/build.json",
        r#"{"job":"build","result":"success"}"#,
    );
    write_fragment(
        temp.path(),
        "fragments/tests.json",
        r#"{"job":"tests","result":"success"}"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("capture")
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--job-file")
        .arg(temp.path().join("fragments").join("*.json"))
        .arg("--output")
        .arg(&output);

    command.assert().success();

    let payload: Value =
        serde_json::from_str(&fs::read_to_string(output).expect("output should be readable"))
            .expect("payload should be valid json");
    assert!(payload["run"]["jobs"].get("build").is_some());
    assert!(payload["run"]["jobs"].get("tests").is_some());
}

#[test]
fn capture_cli_rejects_blank_workflow_names() {
    let temp = tempdir().expect("temp dir should be created");

    write_fragment(
        temp.path(),
        "fragments/build.json",
        r#"{"job":"build","result":"success"}"#,
    );

    let mut command = Command::cargo_bin("github-actionspec").expect("binary should exist");
    command
        .arg("capture")
        .arg("--workflow")
        .arg("   ")
        .arg("--job-file")
        .arg(temp.path().join("fragments").join("build.json"))
        .arg("--output")
        .arg(temp.path().join("actual.json"));

    command.assert().failure().stderr(predicate::str::contains(
        "Capture workflow name must be non-empty.",
    ));
}
