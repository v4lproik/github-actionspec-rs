mod support;

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use github_actionspec_rs::validate::{validate_contract, ValidateContractOptions};
use tempfile::tempdir;

fn matrix_output_schema() -> &'static str {
    "package actionspec\n#WorkflowRun: {\n  workflow: string\n  jobs: [string]: {\n    result: string\n    outputs?: [string]: string\n    matrix?: [string]: string | bool | number | null\n  }\n}\n"
}

fn write_cross_job_fixture(
    temp_root: &Path,
    build_tag: &str,
    published_tag: &str,
) -> (PathBuf, PathBuf, PathBuf) {
    let schema = temp_root.join("schema.cue");
    let contract = temp_root.join("contract.cue");
    let actual = temp_root.join("actual.json");

    std::fs::write(
        &schema,
        "package actionspec\n#WorkflowRun: {\n  workflow: string\n  jobs: [string]: {\n    result: string\n    outputs?: [string]: string\n  }\n}\n",
    )
    .unwrap();
    std::fs::write(
        &contract,
        r#"package actionspec
run: #WorkflowRun & {
  workflow: "release.yml"
  jobs: {
    build: {
      result: "success"
      outputs: {
        image_tag: string
      }
    }
    publish: {
      result: "success"
      outputs: {
        published_tag: run.jobs.build.outputs.image_tag
      }
    }
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        &actual,
        serde_json::to_string(&serde_json::json!({
            "run": {
                "workflow": "release.yml",
                "jobs": {
                    "build": {
                        "result": "success",
                        "outputs": {
                            "image_tag": build_tag,
                        },
                    },
                    "publish": {
                        "result": "success",
                        "outputs": {
                            "published_tag": published_tag,
                        },
                    },
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    (schema, contract, actual)
}

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

#[test]
fn validates_contract_with_multiple_jobs_and_matrix_axes() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_json_validation_fixture(
        temp.path(),
        matrix_output_schema(),
        r#"package actionspec
run: #WorkflowRun & {
  workflow: "build.yml"
  jobs: {
    build: {
      result: "success"
      matrix: {
        app: "build-ts-service"
        target: "linux-amd64"
        shard: 2
        release: true
      }
      outputs: {
        contract_build: "build-ts-service"
      }
    }
    publish: {
      result: "success"
      outputs: {
        image_tag: "v1.2.3"
      }
    }
  }
}
"#,
        &serde_json::json!({
            "run": {
                "workflow": "build.yml",
                "jobs": {
                    "build": {
                        "result": "success",
                        "matrix": {
                            "app": "build-ts-service",
                            "target": "linux-amd64",
                            "shard": 2,
                            "release": true,
                        },
                        "outputs": {
                            "contract_build": "build-ts-service",
                        },
                    },
                    "publish": {
                        "result": "success",
                        "outputs": {
                            "image_tag": "v1.2.3",
                        },
                    },
                }
            }
        }),
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

  grep -q 'target: "linux-amd64"' "$contract"
  grep -q 'shard: 2' "$contract"
  grep -q 'release: true' "$contract"
  grep -q 'image_tag: "v1.2.3"' "$contract"
  grep -q '"target":"linux-amd64"' "$last"
  grep -q '"shard":2' "$last"
  grep -q '"release":true' "$last"
  grep -q '"image_tag":"v1.2.3"' "$last"
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
fn validates_contract_when_optional_outputs_are_omitted() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_json_validation_fixture(
        temp.path(),
        matrix_output_schema(),
        r#"package actionspec
run: #WorkflowRun & {
  workflow: "build.yml"
  jobs: {
    build: {
      result: "success"
      matrix: {
        app: "build-ts-service"
      }
    }
  }
}
"#,
        &serde_json::json!({
            "run": {
                "workflow": "build.yml",
                "jobs": {
                    "build": {
                        "result": "success",
                        "matrix": {
                            "app": "build-ts-service",
                        },
                    },
                }
            }
        }),
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

  grep -q '"app":"build-ts-service"' "$last"
  if grep -q '"outputs"' "$last"; then
    echo "outputs should be omitted for this payload" >&2
    exit 9
  fi
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
fn fails_when_required_matrix_key_is_missing() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_json_validation_fixture(
        temp.path(),
        matrix_output_schema(),
        r#"package actionspec
run: #WorkflowRun & {
  workflow: "build.yml"
  jobs: {
    build: {
      result: "success"
      matrix: {
        app: "build-ts-service"
        target: "linux-amd64"
      }
    }
  }
}
"#,
        &serde_json::json!({
            "run": {
                "workflow": "build.yml",
                "jobs": {
                    "build": {
                        "result": "success",
                        "matrix": {
                            "app": "build-ts-service",
                        },
                    },
                }
            }
        }),
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

  if grep -q 'target: "linux-amd64"' "$contract" && ! grep -q '"target":"linux-amd64"' "$last"; then
    echo "missing matrix.target for jobs.build" >&2
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
        .contains("missing matrix.target for jobs.build"));
}

#[test]
fn fails_when_required_output_key_is_missing() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = support::write_json_validation_fixture(
        temp.path(),
        matrix_output_schema(),
        r#"package actionspec
run: #WorkflowRun & {
  workflow: "build.yml"
  jobs: {
    build: {
      result: "success"
      outputs: {
        contract_build: "build-ts-service"
      }
    }
  }
}
"#,
        &serde_json::json!({
            "run": {
                "workflow": "build.yml",
                "jobs": {
                    "build": {
                        "result": "success",
                        "outputs": {},
                    },
                }
            }
        }),
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

  if grep -q 'contract_build: "build-ts-service"' "$contract" && ! grep -q '"contract_build":"build-ts-service"' "$last"; then
    echo "missing outputs.contract_build for jobs.build" >&2
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
        .contains("missing outputs.contract_build for jobs.build"));
}

#[test]
fn validates_cross_job_output_invariants() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = write_cross_job_fixture(
        temp.path(),
        "ghcr.io/acme/app:sha-123",
        "ghcr.io/acme/app:sha-123",
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

  build_tag="$(grep -o '"image_tag":"[^"]*"' "$last" | head -n1 | cut -d'"' -f4)"
  published_tag="$(grep -o '"published_tag":"[^"]*"' "$last" | head -n1 | cut -d'"' -f4)"
  [ "${build_tag}" = "${published_tag}" ]
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
fn fails_when_cross_job_outputs_diverge() {
    let temp = tempdir().unwrap();
    let (schema, contract, actual) = write_cross_job_fixture(
        temp.path(),
        "ghcr.io/acme/app:sha-123",
        "ghcr.io/acme/app:sha-456",
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

  build_tag="$(grep -o '"image_tag":"[^"]*"' "$last" | head -n1 | cut -d'"' -f4)"
  published_tag="$(grep -o '"published_tag":"[^"]*"' "$last" | head -n1 | cut -d'"' -f4)"
  if [ "${build_tag}" != "${published_tag}" ]; then
    echo "publish job must reuse build image tag: ${build_tag} != ${published_tag}" >&2
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
        .contains("publish job must reuse build image tag: ghcr.io/acme/app:sha-123 != ghcr.io/acme/app:sha-456"));
}
