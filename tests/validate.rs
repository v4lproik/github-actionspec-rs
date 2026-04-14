mod support;

use assert_cmd::Command;
use github_actionspec_rs::validate::{validate_contract, ValidateContractOptions};
use tempfile::tempdir;

#[test]
fn validates_when_cue_vet_succeeds() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_validation_fixture(temp.path(), "demo");

    let env = support::install_fake_cue(&temp, "success");
    let result = validate_contract(ValidateContractOptions {
        schema_paths: vec![schema],
        contract_path: contract,
        actual_paths: vec![actual],
        cwd: Some(temp.path().to_path_buf()),
        env: Some(env),
    });

    assert!(result.is_ok());
}

#[test]
fn validates_through_cli() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_validation_fixture(temp.path(), "demo");
    let env = support::install_fake_cue(&temp, "success");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate")
        .arg("--schema")
        .arg(&schema)
        .arg("--contract")
        .arg(&contract)
        .arg("--actual")
        .arg(&actual);

    command.assert().success();
}

#[test]
fn validates_multiple_actuals_through_cli() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_validation_fixture(temp.path(), "demo");
    let second_actual = temp.path().join("actual-two.json");
    support::write_actual(&second_actual, "demo");
    let env = support::install_fake_cue(&temp, "success");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate")
        .arg("--schema")
        .arg(&schema)
        .arg("--contract")
        .arg(&contract)
        .arg("--actual")
        .arg(&actual)
        .arg("--actual")
        .arg(&second_actual);

    command.assert().success();
}

#[test]
fn validates_globbed_actuals_through_cli() {
    let temp = tempdir().unwrap();
    let (schema, contract, _) = support::write_validation_fixture(temp.path(), "demo");
    let actual_dir = temp.path().join("actuals");
    std::fs::create_dir_all(&actual_dir).unwrap();
    let actual_one = actual_dir.join("actual-one.json");
    let actual_two = actual_dir.join("actual-two.json");
    support::write_actual(&actual_one, "demo");
    support::write_actual(&actual_two, "demo");
    let env = support::install_fake_cue(&temp, "success");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate")
        .arg("--schema")
        .arg(&schema)
        .arg("--contract")
        .arg(&contract)
        .arg("--actual")
        .arg(actual_dir.join("*.json"));

    command.assert().success();
}

#[test]
fn fails_when_cue_vet_fails() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_validation_fixture(temp.path(), "demo");

    let env = support::install_fake_cue(&temp, "failure");
    let error = validate_contract(ValidateContractOptions {
        schema_paths: vec![schema],
        contract_path: contract,
        actual_paths: vec![actual],
        cwd: Some(temp.path().to_path_buf()),
        env: Some(env),
    })
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("cue vet failed with exit code 9"));
}

#[test]
fn validates_contract_with_matrix_and_outputs() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_matrix_output_validation_fixture(
        temp.path(),
        "build.yml",
        "build-ts-service",
        "build-ts-service",
    );

    let env = support::install_fake_cue_script(
        temp.path(),
        r#"#!/bin/sh
set -eu
if [ "$1" = "version" ]; then
  exit 0
fi
if [ "$1" = "vet" ]; then
  last=""
  contract=""
  for arg in "$@"; do
    contract="$last"
    last="$arg"
  done

  grep -q 'app: "build-ts-service"' "$contract"
  grep -q 'contract_build: "build-ts-service"' "$contract"
  grep -q '"app":"build-ts-service"' "$last"
  grep -q '"contract_build":"build-ts-service"' "$last"
  exit 0
fi
exit 1
"#,
    );

    let result = validate_contract(ValidateContractOptions {
        schema_paths: vec![schema],
        contract_path: contract,
        actual_paths: vec![actual],
        cwd: Some(temp.path().to_path_buf()),
        env: Some(env),
    });

    assert!(result.is_ok());
}

#[test]
fn fails_when_matrix_and_output_pattern_diverge() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_matrix_output_validation_fixture(
        temp.path(),
        "build.yml",
        "build-ts-service",
        "contract-build",
    );

    let env = support::install_fake_cue_script(
        temp.path(),
        r#"#!/bin/sh
set -eu
if [ "$1" = "version" ]; then
  exit 0
fi
if [ "$1" = "vet" ]; then
  last=""
  for arg in "$@"; do
    last="$arg"
  done

  app_value="$(grep -o '"app":"[^"]*"' "$last" | head -n1 | cut -d'"' -f4)"
  contract_build_value="$(grep -o '"contract_build":"[^"]*"' "$last" | head -n1 | cut -d'"' -f4)"
  if [ "${app_value}" != "${contract_build_value}" ]; then
    echo "matrix app ${app_value} must match outputs.contract_build ${contract_build_value}" >&2
    exit 9
  fi
  exit 0
fi
exit 1
"#,
    );

    let error = validate_contract(ValidateContractOptions {
        schema_paths: vec![schema],
        contract_path: contract,
        actual_paths: vec![actual],
        cwd: Some(temp.path().to_path_buf()),
        env: Some(env),
    })
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("matrix app build-ts-service must match outputs.contract_build contract-build",));
}
