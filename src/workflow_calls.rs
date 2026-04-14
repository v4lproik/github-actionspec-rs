use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_yaml::{Mapping, Value};
use walkdir::WalkDir;

use crate::errors::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateCallersOptions {
    pub repo_root: PathBuf,
    pub workflows_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateCallersResult {
    pub issues: Vec<CallerValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerValidationIssue {
    pub caller_workflow: PathBuf,
    pub job_id: String,
    pub callee_workflow: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkflowCallContract {
    relative_path: PathBuf,
    inputs: BTreeMap<String, WorkflowCallInput>,
    outputs: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkflowCallInput {
    input_type: WorkflowCallInputType,
    required: bool,
    has_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WorkflowCallInputType {
    String,
    Boolean,
    Number,
}

#[derive(Debug, Clone)]
struct WorkflowFile {
    relative_path: PathBuf,
    document: Value,
}

#[derive(Debug, Clone)]
struct LocalWorkflowCall {
    job_id: String,
    callee_path: PathBuf,
    provided_inputs: BTreeMap<String, Value>,
}

fn string_key(key: &str) -> Value {
    Value::String(key.to_owned())
}

fn mapping_get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(string_key(key))
}

fn value_as_mapping(value: &Value) -> Option<&Mapping> {
    match value {
        Value::Mapping(mapping) => Some(mapping),
        _ => None,
    }
}

fn value_as_str(value: &Value) -> Option<&str> {
    match value {
        Value::String(string) => Some(string),
        _ => None,
    }
}

fn workflows_root(repo_root: &Path, workflows_dir: &Path) -> PathBuf {
    if workflows_dir.is_absolute() {
        workflows_dir.to_path_buf()
    } else {
        repo_root.join(workflows_dir)
    }
}

fn discover_workflow_files(root: &Path) -> Vec<PathBuf> {
    let mut workflow_files = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("yml" | "yaml")
            )
        })
        .collect::<Vec<_>>();
    workflow_files.sort();
    workflow_files
}

fn load_workflow_file(repo_root: &Path, path: &Path) -> Result<WorkflowFile, AppError> {
    let relative_path = path
        .strip_prefix(repo_root)
        .map(PathBuf::from)
        .unwrap_or_else(|_| path.to_path_buf());
    let document = serde_yaml::from_str(&fs::read_to_string(path)?)?;

    Ok(WorkflowFile {
        relative_path,
        document,
    })
}

fn parse_input_type(value: Option<&str>) -> WorkflowCallInputType {
    match value {
        Some("boolean") => WorkflowCallInputType::Boolean,
        Some("number") => WorkflowCallInputType::Number,
        _ => WorkflowCallInputType::String,
    }
}

fn parse_bool(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(true)))
}

fn parse_workflow_call_contract(file: &WorkflowFile) -> Option<WorkflowCallContract> {
    let root = value_as_mapping(&file.document)?;
    let on = value_as_mapping(mapping_get(root, "on")?)?;
    let workflow_call = value_as_mapping(mapping_get(on, "workflow_call")?)?;

    let inputs = mapping_get(workflow_call, "inputs")
        .and_then(value_as_mapping)
        .map(|inputs| {
            inputs
                .iter()
                .filter_map(|(name, value)| {
                    let name = value_as_str(name)?;
                    let spec = value_as_mapping(value)?;
                    Some((
                        name.to_owned(),
                        WorkflowCallInput {
                            input_type: parse_input_type(
                                mapping_get(spec, "type").and_then(value_as_str),
                            ),
                            required: parse_bool(mapping_get(spec, "required")),
                            has_default: mapping_get(spec, "default").is_some(),
                        },
                    ))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    let outputs = mapping_get(workflow_call, "outputs")
        .and_then(value_as_mapping)
        .map(|outputs| {
            outputs
                .keys()
                .filter_map(value_as_str)
                .map(ToOwned::to_owned)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();

    Some(WorkflowCallContract {
        relative_path: file.relative_path.clone(),
        inputs,
        outputs,
    })
}

fn normalize_local_workflow_use(uses: &str) -> Option<PathBuf> {
    let normalized = uses.strip_prefix("./").unwrap_or(uses);
    normalized
        .starts_with(".github/workflows/")
        .then(|| PathBuf::from(normalized))
}

fn discover_local_workflow_calls(file: &WorkflowFile) -> Vec<LocalWorkflowCall> {
    let Some(root) = value_as_mapping(&file.document) else {
        return Vec::new();
    };
    let Some(jobs) = mapping_get(root, "jobs").and_then(value_as_mapping) else {
        return Vec::new();
    };

    jobs.iter()
        .filter_map(|(job_id, value)| {
            let job_id = value_as_str(job_id)?;
            let job = value_as_mapping(value)?;
            let uses = mapping_get(job, "uses").and_then(value_as_str)?;
            let callee_path = normalize_local_workflow_use(uses)?;
            let provided_inputs = mapping_get(job, "with")
                .and_then(value_as_mapping)
                .map(|inputs| {
                    inputs
                        .iter()
                        .filter_map(|(key, value)| {
                            Some((value_as_str(key)?.to_owned(), value.clone()))
                        })
                        .collect::<BTreeMap<_, _>>()
                })
                .unwrap_or_default();

            Some(LocalWorkflowCall {
                job_id: job_id.to_owned(),
                callee_path,
                provided_inputs,
            })
        })
        .collect()
}

fn needs_output_regex() -> Result<Regex, AppError> {
    Regex::new(r"needs\.([A-Za-z0-9_-]+)\.outputs\.([A-Za-z0-9_-]+)").map_err(AppError::from)
}

fn collect_output_references(value: &Value, references: &mut Vec<(String, String)>, regex: &Regex) {
    match value {
        Value::String(string) => {
            for captures in regex.captures_iter(string) {
                if let (Some(job_id), Some(output_name)) = (captures.get(1), captures.get(2)) {
                    references.push((job_id.as_str().to_owned(), output_name.as_str().to_owned()));
                }
            }
        }
        Value::Sequence(sequence) => {
            for item in sequence {
                collect_output_references(item, references, regex);
            }
        }
        Value::Mapping(mapping) => {
            for (key, value) in mapping {
                collect_output_references(key, references, regex);
                collect_output_references(value, references, regex);
            }
        }
        _ => {}
    }
}

fn type_matches_literal(value: &Value, input_type: &WorkflowCallInputType) -> bool {
    match (value, input_type) {
        (Value::Bool(_), WorkflowCallInputType::Boolean) => true,
        (Value::Number(_), WorkflowCallInputType::Number) => true,
        (Value::String(string), _) if string.contains("${{") => true,
        (Value::String(_), WorkflowCallInputType::String) => true,
        (Value::String(string), WorkflowCallInputType::Boolean) => {
            matches!(string.as_str(), "true" | "false")
        }
        (Value::String(string), WorkflowCallInputType::Number) => string.parse::<f64>().is_ok(),
        _ => false,
    }
}

fn describe_input_type(input_type: &WorkflowCallInputType) -> &'static str {
    match input_type {
        WorkflowCallInputType::String => "string",
        WorkflowCallInputType::Boolean => "boolean",
        WorkflowCallInputType::Number => "number",
    }
}

fn build_call_map(calls: &[LocalWorkflowCall]) -> BTreeMap<String, PathBuf> {
    calls
        .iter()
        .map(|call| (call.job_id.clone(), call.callee_path.clone()))
        .collect()
}

pub fn validate_workflow_callers(
    options: ValidateCallersOptions,
) -> Result<ValidateCallersResult, AppError> {
    let workflows_root = workflows_root(&options.repo_root, &options.workflows_dir);
    let workflow_files = discover_workflow_files(&workflows_root)
        .into_iter()
        .map(|path| load_workflow_file(&options.repo_root, &path))
        .collect::<Result<Vec<_>, _>>()?;
    let contracts = workflow_files
        .iter()
        .filter_map(parse_workflow_call_contract)
        .map(|contract| (contract.relative_path.clone(), contract))
        .collect::<BTreeMap<_, _>>();

    let regex = needs_output_regex()?;
    let mut issues = Vec::new();

    for workflow in &workflow_files {
        let local_calls = discover_local_workflow_calls(workflow);
        let call_map = build_call_map(&local_calls);

        for call in &local_calls {
            let Some(contract) = contracts.get(&call.callee_path) else {
                issues.push(CallerValidationIssue {
                    caller_workflow: workflow.relative_path.clone(),
                    job_id: call.job_id.clone(),
                    callee_workflow: call.callee_path.clone(),
                    message: "local reusable workflow is missing a workflow_call contract"
                        .to_owned(),
                });
                continue;
            };

            for (input_name, input_spec) in &contract.inputs {
                if input_spec.required
                    && !input_spec.has_default
                    && !call.provided_inputs.contains_key(input_name)
                {
                    issues.push(CallerValidationIssue {
                        caller_workflow: workflow.relative_path.clone(),
                        job_id: call.job_id.clone(),
                        callee_workflow: call.callee_path.clone(),
                        message: format!("missing required input `{input_name}`"),
                    });
                }
            }

            for (input_name, value) in &call.provided_inputs {
                let Some(input_spec) = contract.inputs.get(input_name) else {
                    issues.push(CallerValidationIssue {
                        caller_workflow: workflow.relative_path.clone(),
                        job_id: call.job_id.clone(),
                        callee_workflow: call.callee_path.clone(),
                        message: format!("unexpected input `{input_name}`"),
                    });
                    continue;
                };

                if !type_matches_literal(value, &input_spec.input_type) {
                    issues.push(CallerValidationIssue {
                        caller_workflow: workflow.relative_path.clone(),
                        job_id: call.job_id.clone(),
                        callee_workflow: call.callee_path.clone(),
                        message: format!(
                            "input `{input_name}` expects a {} value",
                            describe_input_type(&input_spec.input_type)
                        ),
                    });
                }
            }
        }

        let mut output_references = Vec::new();
        collect_output_references(&workflow.document, &mut output_references, &regex);
        for (job_id, output_name) in output_references {
            let Some(callee_path) = call_map.get(&job_id) else {
                continue;
            };
            let Some(contract) = contracts.get(callee_path) else {
                continue;
            };

            if !contract.outputs.contains(&output_name) {
                issues.push(CallerValidationIssue {
                    caller_workflow: workflow.relative_path.clone(),
                    job_id,
                    callee_workflow: callee_path.clone(),
                    message: format!("references missing reusable workflow output `{output_name}`"),
                });
            }
        }
    }

    Ok(ValidateCallersResult { issues })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_workflow(repo_root: &Path, relative_path: &str, contents: &str) {
        let path = repo_root.join(relative_path);
        let parent = path.parent().expect("workflow file should have a parent");
        fs::create_dir_all(parent).expect("workflow directory should be created");
        fs::write(path, contents).expect("workflow fixture should be written");
    }

    fn validate(repo_root: &Path) -> ValidateCallersResult {
        validate_workflow_callers(ValidateCallersOptions {
            repo_root: repo_root.to_path_buf(),
            workflows_dir: PathBuf::from(".github/workflows"),
        })
        .expect("caller validation should succeed")
    }

    #[test]
    fn validates_local_reusable_workflow_inputs_and_outputs() {
        let repo = tempdir().expect("temp dir should be created");
        write_workflow(
            repo.path(),
            ".github/workflows/build-contract.yml",
            r#"on:
  workflow_call:
    inputs:
      environment:
        type: string
        required: true
      push:
        type: boolean
        required: false
    outputs:
      validator_runtime_tag:
        value: ${{ jobs.build.outputs.validator_runtime_tag }}
jobs:
  build:
    runs-on: ubuntu-latest
"#,
        );
        write_workflow(
            repo.path(),
            ".github/workflows/test-e2e.yml",
            r#"on:
  workflow_call:
jobs:
  build-contract:
    uses: ./.github/workflows/build-contract.yml
    with:
      environment: test
      push: true
  e2e:
    runs-on: ubuntu-latest
    needs: [build-contract]
    steps:
      - run: echo "${{ needs.build-contract.outputs.validator_runtime_tag }}"
"#,
        );

        let result = validate(repo.path());

        assert!(result.issues.is_empty());
    }

    #[test]
    fn reports_missing_required_inputs_and_unknown_outputs() {
        let repo = tempdir().expect("temp dir should be created");
        write_workflow(
            repo.path(),
            ".github/workflows/build-contract.yml",
            r#"on:
  workflow_call:
    inputs:
      environment:
        type: string
        required: true
    outputs:
      validator_runtime_tag:
        value: ${{ jobs.build.outputs.validator_runtime_tag }}
jobs:
  build:
    runs-on: ubuntu-latest
"#,
        );
        write_workflow(
            repo.path(),
            ".github/workflows/test-e2e.yml",
            r#"on:
  workflow_call:
jobs:
  build-contract:
    uses: ./.github/workflows/build-contract.yml
  e2e:
    runs-on: ubuntu-latest
    needs: [build-contract]
    steps:
      - run: echo "${{ needs.build-contract.outputs.missing_tag }}"
"#,
        );

        let result = validate(repo.path());

        assert_eq!(result.issues.len(), 2);
        assert!(result.issues.iter().any(|issue| issue
            .message
            .contains("missing required input `environment`")));
        assert!(result.issues.iter().any(|issue| issue
            .message
            .contains("missing reusable workflow output `missing_tag`")));
    }

    #[test]
    fn reports_unexpected_inputs_and_type_mismatches() {
        let repo = tempdir().expect("temp dir should be created");
        write_workflow(
            repo.path(),
            ".github/workflows/build-service.yml",
            r#"on:
  workflow_call:
    inputs:
      changed:
        type: boolean
        required: false
      node-version:
        type: string
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
  push:
jobs:
  build-service:
    uses: ./.github/workflows/build-service.yml
    with:
      changed: definitely
      node-version: 20
      unknown-input: true
"#,
        );

        let result = validate(repo.path());

        assert_eq!(result.issues.len(), 3);
        assert!(result.issues.iter().any(|issue| issue
            .message
            .contains("input `changed` expects a boolean value")));
        assert!(result.issues.iter().any(|issue| issue
            .message
            .contains("input `node-version` expects a string value")));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.message.contains("unexpected input `unknown-input`")));
    }
}
