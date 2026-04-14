use std::path::{Path, PathBuf};

use github_actionspec_rs::validate::{validate_contract, ValidateContractOptions};
use tempfile::tempdir;

fn repo_schema_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn write_contract(path: &Path, contract_build: &str) {
    std::fs::write(
        path,
        format!(
            r#"package actionspec

workflow: "build.yml"

run: #Declaration.run & {{
  workflow: workflow
  jobs: {{
    build: {{
      result: "success"
      matrix: {{
        app: "build-ts-service"
        target: "linux-amd64"
      }}
      outputs: {{
        contract_build: "{contract_build}"
      }}
    }}
  }}
}}
"#
        ),
    )
    .unwrap();
}

fn write_actual(path: &Path, contract_build: &str) {
    std::fs::write(
        path,
        format!(
            r#"{{
  "run": {{
    "workflow": "build.yml",
    "jobs": {{
      "build": {{
        "result": "success",
        "matrix": {{
          "app": "build-ts-service",
          "target": "linux-amd64"
        }},
        "outputs": {{
          "contract_build": "{contract_build}"
        }}
      }}
    }}
  }}
}}"#
        ),
    )
    .unwrap();
}

#[test]
fn real_cue_validates_matrix_and_outputs_against_repo_schema() {
    let temp = tempdir().unwrap();
    let contract = temp.path().join("contract.cue");
    let actual = temp.path().join("actual.json");
    write_contract(&contract, "build-ts-service");
    write_actual(&actual, "build-ts-service");

    let result = validate_contract(ValidateContractOptions {
        schema_paths: vec![
            repo_schema_path("schema/workflow_run.cue"),
            repo_schema_path("schema/declaration.cue"),
        ],
        contract_path: contract,
        actual_paths: vec![actual],
        cwd: Some(temp.path().to_path_buf()),
        env: None,
    });

    assert!(result.is_ok());
}

#[test]
fn real_cue_rejects_diverging_matrix_and_output_values() {
    let temp = tempdir().unwrap();
    let contract = temp.path().join("contract.cue");
    let actual = temp.path().join("actual.json");
    write_contract(&contract, "build-ts-service");
    write_actual(&actual, "contract-build");

    let error = validate_contract(ValidateContractOptions {
        schema_paths: vec![
            repo_schema_path("schema/workflow_run.cue"),
            repo_schema_path("schema/declaration.cue"),
        ],
        contract_path: contract,
        actual_paths: vec![actual],
        cwd: Some(temp.path().to_path_buf()),
        env: None,
    })
    .unwrap_err();

    let message = error.to_string();
    assert!(message.contains("cue vet failed"));
    assert!(message.contains("contract_build"));
    assert!(message.contains("build-ts-service"));
}
