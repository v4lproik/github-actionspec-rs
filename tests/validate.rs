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
        actual_path: actual,
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
fn fails_when_cue_vet_fails() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_validation_fixture(temp.path(), "demo");

    let env = support::install_fake_cue(&temp, "failure");
    let error = validate_contract(ValidateContractOptions {
        schema_paths: vec![schema],
        contract_path: contract,
        actual_path: actual,
        cwd: Some(temp.path().to_path_buf()),
        env: Some(env),
    })
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("cue vet failed with exit code 9"));
}
