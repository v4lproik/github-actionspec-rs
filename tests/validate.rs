mod support;

use github_actionspec_rs::validate::{validate_contract, ValidateContractOptions};
use tempfile::tempdir;

#[test]
fn validates_when_cue_vet_succeeds() {
    let temp = tempdir().unwrap();
    let schema = temp.path().join("schema.cue");
    let contract = temp.path().join("contract.cue");
    let actual = temp.path().join("actual.json");

    std::fs::write(&schema, "package actionspec\n#WorkflowRun: {workflow: string, jobs: [string]: {result: string}}\n").unwrap();
    std::fs::write(&contract, "package actionspec\nrun: #WorkflowRun & {workflow: \"demo\", jobs: {build: {result: \"success\"}}}\n").unwrap();
    std::fs::write(&actual, "{\"run\":{\"workflow\":\"demo\",\"jobs\":{\"build\":{\"result\":\"success\"}}}}")
        .unwrap();

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
fn fails_when_cue_vet_fails() {
    let temp = tempdir().unwrap();
    let schema = temp.path().join("schema.cue");
    let contract = temp.path().join("contract.cue");
    let actual = temp.path().join("actual.json");

    std::fs::write(&schema, "package actionspec\n#WorkflowRun: {workflow: string, jobs: [string]: {result: string}}\n").unwrap();
    std::fs::write(&contract, "package actionspec\nrun: #WorkflowRun & {workflow: \"demo\", jobs: {build: {result: \"success\"}}}\n").unwrap();
    std::fs::write(&actual, "{\"run\":{\"workflow\":\"demo\",\"jobs\":{\"build\":{\"result\":\"success\"}}}}")
        .unwrap();

    let env = support::install_fake_cue(&temp, "failure");
    let error = validate_contract(ValidateContractOptions {
        schema_paths: vec![schema],
        contract_path: contract,
        actual_path: actual,
        cwd: Some(temp.path().to_path_buf()),
        env: Some(env),
    })
    .unwrap_err();

    assert!(error.to_string().contains("cue vet failed with exit code 9"));
}
