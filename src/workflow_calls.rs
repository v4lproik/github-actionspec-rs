use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
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
    pub report: WorkflowCallReport,
    pub failed_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowCallReport {
    pub workflows: Vec<WorkflowCallAnalysis>,
    pub issues: Vec<CallerValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowCallAnalysis {
    pub workflow_path: PathBuf,
    pub calls: Vec<WorkflowCallAnalysisCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowCallAnalysisCall {
    pub job_id: String,
    pub callee_workflow: PathBuf,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub provided_inputs: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referenced_outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

fn workflow_jobs(document: &Value) -> Option<&Mapping> {
    value_as_mapping(document)
        .and_then(|root| mapping_get(root, "jobs"))
        .and_then(value_as_mapping)
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

fn workflow_call_mapping(document: &Value) -> Option<&Mapping> {
    value_as_mapping(document)
        .and_then(|root| mapping_get(root, "on"))
        .and_then(value_as_mapping)
        .and_then(|on| mapping_get(on, "workflow_call"))
        .and_then(value_as_mapping)
}

fn parse_workflow_call_contract(file: &WorkflowFile) -> Option<WorkflowCallContract> {
    let workflow_call = workflow_call_mapping(&file.document)?;

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
    let Some(jobs) = workflow_jobs(&file.document) else {
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
    static NEEDS_OUTPUT_REGEX: OnceLock<Result<Regex, regex::Error>> = OnceLock::new();

    NEEDS_OUTPUT_REGEX
        .get_or_init(|| Regex::new(r"needs\.([A-Za-z0-9_-]+)\.outputs\.([A-Za-z0-9_-]+)"))
        .clone()
        .map_err(AppError::from)
}

fn collect_output_references(
    value: &Value,
    references: &mut BTreeMap<String, BTreeSet<String>>,
    regex: &Regex,
) {
    match value {
        Value::String(string) => {
            for captures in regex.captures_iter(string) {
                if let (Some(job_id), Some(output_name)) = (captures.get(1), captures.get(2)) {
                    references
                        .entry(job_id.as_str().to_owned())
                        .or_default()
                        .insert(output_name.as_str().to_owned());
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

fn referenced_outputs_for_job(
    output_references: &BTreeMap<String, BTreeSet<String>>,
    job_id: &str,
) -> Vec<String> {
    output_references
        .get(job_id)
        .map(|outputs| outputs.iter().cloned().collect())
        .unwrap_or_default()
}

fn push_issue(
    issues: &mut Vec<CallerValidationIssue>,
    workflow: &WorkflowFile,
    job_id: &str,
    callee_workflow: &Path,
    message: impl Into<String>,
) {
    issues.push(CallerValidationIssue {
        caller_workflow: workflow.relative_path.clone(),
        job_id: job_id.to_owned(),
        callee_workflow: callee_workflow.to_path_buf(),
        message: message.into(),
    });
}

fn record_issue(
    issues: &mut Vec<CallerValidationIssue>,
    call_issues: &mut Vec<String>,
    workflow: &WorkflowFile,
    job_id: &str,
    callee_workflow: &Path,
    message: impl Into<String>,
) {
    let message = message.into();
    push_issue(issues, workflow, job_id, callee_workflow, message.clone());
    call_issues.push(message);
}

pub fn write_workflow_call_report(
    report: &WorkflowCallReport,
    path: &Path,
) -> Result<(), AppError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(report)?)?;
    Ok(())
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
    let mut workflows = Vec::new();

    for workflow in &workflow_files {
        let local_calls = discover_local_workflow_calls(workflow);
        if local_calls.is_empty() {
            continue;
        }

        let mut output_references = BTreeMap::new();
        // Reusable workflow outputs are consumed via `${{ needs.<job>.outputs.<name> }}` in
        // arbitrary strings, so we scan the full YAML document recursively instead of trying
        // to maintain a bespoke AST for every expression-bearing field.
        collect_output_references(&workflow.document, &mut output_references, &regex);
        let mut calls = Vec::with_capacity(local_calls.len());

        for call in &local_calls {
            let referenced_outputs = referenced_outputs_for_job(&output_references, &call.job_id);
            let mut call_issues = Vec::new();

            let Some(contract) = contracts.get(&call.callee_path) else {
                record_issue(
                    &mut issues,
                    &mut call_issues,
                    workflow,
                    &call.job_id,
                    &call.callee_path,
                    "local reusable workflow is missing a workflow_call contract",
                );
                calls.push(WorkflowCallAnalysisCall {
                    job_id: call.job_id.clone(),
                    callee_workflow: call.callee_path.clone(),
                    provided_inputs: call.provided_inputs.clone(),
                    referenced_outputs,
                    issues: call_issues,
                });
                continue;
            };

            for (input_name, input_spec) in &contract.inputs {
                if input_spec.required
                    && !input_spec.has_default
                    && !call.provided_inputs.contains_key(input_name)
                {
                    record_issue(
                        &mut issues,
                        &mut call_issues,
                        workflow,
                        &call.job_id,
                        &call.callee_path,
                        format!("missing required input `{input_name}`"),
                    );
                }
            }

            for (input_name, value) in &call.provided_inputs {
                let Some(input_spec) = contract.inputs.get(input_name) else {
                    record_issue(
                        &mut issues,
                        &mut call_issues,
                        workflow,
                        &call.job_id,
                        &call.callee_path,
                        format!("unexpected input `{input_name}`"),
                    );
                    continue;
                };

                if !type_matches_literal(value, &input_spec.input_type) {
                    record_issue(
                        &mut issues,
                        &mut call_issues,
                        workflow,
                        &call.job_id,
                        &call.callee_path,
                        format!(
                            "input `{input_name}` expects a {} value",
                            describe_input_type(&input_spec.input_type)
                        ),
                    );
                }
            }

            for output_name in &referenced_outputs {
                if !contract.outputs.contains(output_name) {
                    record_issue(
                        &mut issues,
                        &mut call_issues,
                        workflow,
                        &call.job_id,
                        &call.callee_path,
                        format!("references missing reusable workflow output `{output_name}`"),
                    );
                }
            }

            calls.push(WorkflowCallAnalysisCall {
                job_id: call.job_id.clone(),
                callee_workflow: call.callee_path.clone(),
                provided_inputs: call.provided_inputs.clone(),
                referenced_outputs,
                issues: call_issues,
            });
        }

        workflows.push(WorkflowCallAnalysis {
            workflow_path: workflow.relative_path.clone(),
            calls,
        });
    }

    issues.sort();
    issues.dedup();

    let failed_count = issues.len();

    Ok(ValidateCallersResult {
        report: WorkflowCallReport { workflows, issues },
        failed_count,
    })
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

        assert_eq!(result.failed_count, 0);
        assert!(result.report.issues.is_empty());
        assert_eq!(result.report.workflows.len(), 1);
        assert_eq!(result.report.workflows[0].calls.len(), 1);
        assert_eq!(
            result.report.workflows[0].calls[0].referenced_outputs,
            vec!["validator_runtime_tag".to_owned()]
        );
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

        assert_eq!(result.failed_count, 2);
        assert_eq!(result.report.issues.len(), 2);
        assert!(result.report.issues.iter().any(|issue| issue
            .message
            .contains("missing required input `environment`")));
        assert!(result.report.issues.iter().any(|issue| issue
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

        assert_eq!(result.failed_count, 3);
        assert!(result.report.issues.iter().any(|issue| issue
            .message
            .contains("input `changed` expects a boolean value")));
        assert!(result.report.issues.iter().any(|issue| issue
            .message
            .contains("input `node-version` expects a string value")));
        assert!(result
            .report
            .issues
            .iter()
            .any(|issue| issue.message.contains("unexpected input `unknown-input`")));
    }

    #[test]
    fn accepts_expression_inputs_for_typed_reusable_workflow_parameters() {
        let repo = tempdir().expect("temp dir should be created");
        write_workflow(
            repo.path(),
            ".github/workflows/reusable-check.yml",
            r#"on:
  workflow_call:
    inputs:
      changed:
        type: boolean
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
  pull_request:
jobs:
  build:
    uses: ./.github/workflows/reusable-check.yml
    with:
      changed: ${{ github.event_name == 'pull_request' }}
"#,
        );

        let result = validate(repo.path());

        assert_eq!(result.failed_count, 0);
        assert!(result.report.issues.is_empty());
    }

    #[test]
    fn reports_local_reusable_workflows_without_workflow_call_contracts() {
        let repo = tempdir().expect("temp dir should be created");
        write_workflow(
            repo.path(),
            ".github/workflows/reusable-check.yml",
            r#"on:
  push:
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
    uses: ./.github/workflows/reusable-check.yml
"#,
        );

        let result = validate(repo.path());

        assert_eq!(result.failed_count, 1);
        assert_eq!(result.report.issues.len(), 1);
        assert!(result.report.issues[0]
            .message
            .contains("missing a workflow_call contract"));
    }

    #[test]
    fn deduplicates_repeated_missing_output_references_per_call() {
        let repo = tempdir().expect("temp dir should be created");
        write_workflow(
            repo.path(),
            ".github/workflows/reusable-check.yml",
            r#"on:
  workflow_call:
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
    uses: ./.github/workflows/reusable-check.yml
  summarize:
    runs-on: ubuntu-latest
    needs: [build]
    steps:
      - run: echo "${{ needs.build.outputs.missing_tag }}"
      - run: echo "${{ needs.build.outputs.missing_tag }}"
"#,
        );

        let result = validate(repo.path());

        assert_eq!(result.failed_count, 1);
        assert_eq!(result.report.issues.len(), 1);
        assert_eq!(result.report.workflows.len(), 1);
        assert_eq!(
            result.report.workflows[0].calls[0].referenced_outputs,
            vec!["missing_tag".to_owned()]
        );
        assert_eq!(
            result.report.workflows[0].calls[0].issues,
            vec!["references missing reusable workflow output `missing_tag`".to_owned()]
        );
    }
}
