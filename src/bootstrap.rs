use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value as JsonValue;
use serde_yaml::{Mapping, Value as YamlValue};

use crate::errors::AppError;
use crate::fs_utils::write_pretty_json_file;
use crate::types::{WorkflowJobRecord, WorkflowRunEnvelope, WorkflowRunRecord, WorkflowStepRecord};

const DEFAULT_WORKFLOWS_DIR: &str = ".github/workflows";
const DEFAULT_FIXTURES_DIR: &str = "tests/fixtures";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapOptions {
    pub repo_root: PathBuf,
    pub workflow: String,
    pub actual: Option<PathBuf>,
    pub declarations_dir: PathBuf,
    pub workflows_dir: PathBuf,
    pub fixtures_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapResult {
    pub workflow: String,
    pub workflow_path: PathBuf,
    pub declaration_path: PathBuf,
    pub actual_path: PathBuf,
    pub snippet_path: PathBuf,
    pub seeded_from_actual: bool,
}

fn string_key(key: &str) -> YamlValue {
    YamlValue::String(key.to_owned())
}

fn mapping_get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a YamlValue> {
    mapping.get(string_key(key))
}

fn value_as_mapping(value: &YamlValue) -> Option<&Mapping> {
    match value {
        YamlValue::Mapping(mapping) => Some(mapping),
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

fn workflow_jobs(document: &YamlValue) -> Option<&Mapping> {
    value_as_mapping(document)
        .and_then(|root| mapping_get(root, "jobs"))
        .and_then(value_as_mapping)
}

fn slugify_workflow_name(workflow: &str) -> String {
    Path::new(workflow)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("workflow")
        .to_owned()
}

fn normalize_workflow_name(workflow: &str) -> String {
    Path::new(workflow)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(workflow)
        .to_owned()
}

fn resolve_workflow_path(repo_root: &Path, workflows_dir: &Path, workflow: &str) -> PathBuf {
    let workflow_path = Path::new(workflow);
    if workflow_path.is_absolute() {
        workflow_path.to_path_buf()
    } else {
        let repo_relative = repo_root.join(workflow_path);
        if repo_relative.is_file() {
            repo_relative
        } else {
            workflows_root(repo_root, workflows_dir).join(workflow_path)
        }
    }
}

fn assert_writable_output(path: &Path, force: bool) -> Result<(), AppError> {
    if path.exists() && !force {
        return Err(AppError::BootstrapOutputExists(path.to_path_buf()));
    }

    Ok(())
}

fn write_text_file(path: &Path, contents: &str) -> Result<(), AppError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

fn load_workflow_document(path: &Path) -> Result<YamlValue, AppError> {
    Ok(serde_yaml::from_str(&fs::read_to_string(path)?)?)
}

fn synthesize_baseline_payload(
    workflow: &str,
    workflow_path: &Path,
) -> Result<WorkflowRunEnvelope, AppError> {
    let document = load_workflow_document(workflow_path)?;
    let jobs = workflow_jobs(&document).ok_or_else(|| AppError::BootstrapWorkflowHasNoJobs {
        workflow: workflow.to_owned(),
        path: workflow_path.to_path_buf(),
    })?;

    if jobs.is_empty() {
        return Err(AppError::BootstrapWorkflowHasNoJobs {
            workflow: workflow.to_owned(),
            path: workflow_path.to_path_buf(),
        });
    }

    let jobs = jobs
        .keys()
        .filter_map(|job| match job {
            YamlValue::String(job) if !job.trim().is_empty() => Some(job.clone()),
            _ => None,
        })
        .map(|job| {
            (
                job,
                WorkflowJobRecord {
                    result: "success".to_owned(),
                    outputs: None,
                    matrix: None,
                    steps: None,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    Ok(WorkflowRunEnvelope {
        run: WorkflowRunRecord {
            workflow: workflow.to_owned(),
            ref_name: None,
            inputs: None,
            jobs,
        },
    })
}

fn load_actual_payload(
    path: &Path,
    expected_workflow: &str,
) -> Result<WorkflowRunEnvelope, AppError> {
    let payload: WorkflowRunEnvelope = serde_json::from_str(&fs::read_to_string(path)?)?;
    if payload.run.workflow != expected_workflow {
        return Err(AppError::BootstrapActualWorkflowMismatch {
            path: path.to_path_buf(),
            expected: expected_workflow.to_owned(),
            found: payload.run.workflow.clone(),
        });
    }

    Ok(payload)
}

fn cue_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

fn render_json_value(value: &JsonValue, indent: usize) -> String {
    match value {
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_) => {
            value.to_string()
        }
        JsonValue::Array(values) => {
            if values.is_empty() {
                "[]".to_owned()
            } else {
                let nested_indent = indent + 2;
                let items = values
                    .iter()
                    .map(|value| {
                        format!(
                            "{}{}",
                            " ".repeat(nested_indent),
                            render_json_value(value, nested_indent)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");
                format!("[\n{items}\n{}]", " ".repeat(indent))
            }
        }
        JsonValue::Object(object) => {
            if object.is_empty() {
                "{}".to_owned()
            } else {
                let nested_indent = indent + 2;
                let fields = object
                    .iter()
                    .map(|(key, value)| {
                        format!(
                            "{}{}: {}",
                            " ".repeat(nested_indent),
                            cue_string(key),
                            render_json_value(value, nested_indent)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("{{\n{fields}\n{}}}", " ".repeat(indent))
            }
        }
    }
}

fn render_optional_string_map(
    field_name: &str,
    map: Option<&BTreeMap<String, String>>,
    indent: usize,
) -> Option<String> {
    let map = map.filter(|map| !map.is_empty())?;
    let nested_indent = indent + 2;
    let fields = map
        .iter()
        .map(|(key, value)| {
            format!(
                "{}{}: {}",
                " ".repeat(nested_indent),
                cue_string(key),
                cue_string(value)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Some(format!(
        "{}{}: {{\n{}\n{}}}",
        " ".repeat(indent),
        field_name,
        fields,
        " ".repeat(indent)
    ))
}

fn render_optional_inputs(
    inputs: Option<&BTreeMap<String, Option<String>>>,
    indent: usize,
) -> Option<String> {
    let inputs = inputs.filter(|inputs| !inputs.is_empty())?;
    let nested_indent = indent + 2;
    let fields = inputs
        .iter()
        .map(|(key, value)| {
            let value = match value {
                Some(value) => cue_string(value),
                None => "null".to_owned(),
            };
            format!(
                "{}{}: {}",
                " ".repeat(nested_indent),
                cue_string(key),
                value
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Some(format!(
        "{}inputs: {{\n{}\n{}}}",
        " ".repeat(indent),
        fields,
        " ".repeat(indent)
    ))
}

fn render_optional_matrix(
    matrix: Option<&BTreeMap<String, JsonValue>>,
    indent: usize,
) -> Option<String> {
    let matrix = matrix.filter(|matrix| !matrix.is_empty())?;
    let nested_indent = indent + 2;
    let fields = matrix
        .iter()
        .map(|(key, value)| {
            format!(
                "{}{}: {}",
                " ".repeat(nested_indent),
                cue_string(key),
                render_json_value(value, nested_indent)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Some(format!(
        "{}matrix: {{\n{}\n{}}}",
        " ".repeat(indent),
        fields,
        " ".repeat(indent)
    ))
}

fn render_optional_steps(
    steps: Option<&BTreeMap<String, WorkflowStepRecord>>,
    indent: usize,
) -> Option<String> {
    let steps = steps.filter(|steps| !steps.is_empty())?;
    let nested_indent = indent + 2;
    let step_blocks = steps
        .iter()
        .map(|(step_name, step)| {
            let mut step_lines = Vec::new();
            if let Some(conclusion) = &step.conclusion {
                step_lines.push(format!(
                    "{}conclusion: {}",
                    " ".repeat(nested_indent + 2),
                    cue_string(conclusion)
                ));
            }
            if let Some(outputs) =
                render_optional_string_map("outputs", step.outputs.as_ref(), nested_indent + 2)
            {
                step_lines.push(outputs);
            }
            format!(
                "{}{}: {{\n{}\n{}}}",
                " ".repeat(nested_indent),
                cue_string(step_name),
                step_lines.join("\n"),
                " ".repeat(nested_indent)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Some(format!(
        "{}steps: {{\n{}\n{}}}",
        " ".repeat(indent),
        step_blocks,
        " ".repeat(indent)
    ))
}

fn render_job_block(job_name: &str, job: &WorkflowJobRecord, indent: usize) -> String {
    let mut lines = vec![format!(
        "{}result: {}",
        " ".repeat(indent + 2),
        cue_string(&job.result)
    )];

    if let Some(outputs) = render_optional_string_map("outputs", job.outputs.as_ref(), indent + 2) {
        lines.push(outputs);
    }
    if let Some(matrix) = render_optional_matrix(job.matrix.as_ref(), indent + 2) {
        lines.push(matrix);
    }
    if let Some(steps) = render_optional_steps(job.steps.as_ref(), indent + 2) {
        lines.push(steps);
    }

    format!(
        "{}{}: {{\n{}\n{}}}",
        " ".repeat(indent),
        cue_string(job_name),
        lines.join("\n"),
        " ".repeat(indent)
    )
}

fn render_contract(workflow: &str, payload: &WorkflowRunEnvelope) -> String {
    let mut sections = vec![
        format!("workflow: {}", cue_string(workflow)),
        String::new(),
        "run: #Declaration.run & {".to_owned(),
        "  workflow: workflow".to_owned(),
    ];

    if let Some(ref_name) = &payload.run.ref_name {
        sections.push(format!("  ref: {}", cue_string(ref_name)));
    }
    if let Some(inputs) = render_optional_inputs(payload.run.inputs.as_ref(), 2) {
        sections.push(inputs);
    }

    let jobs = payload
        .run
        .jobs
        .iter()
        .map(|(job_name, job)| render_job_block(job_name, job, 4))
        .collect::<Vec<_>>()
        .join("\n");
    sections.push(format!("  jobs: {{\n{}\n  }}", jobs));
    sections.push("}".to_owned());

    format!(
        "package actionspec\n\n// Starter contract generated by `github-actionspec bootstrap`.\n// It matches the baseline payload exactly so repositories can tighten the rules incrementally.\n// Tighten refs, result unions, matrix keys, and outputs as the workflow stabilizes.\n{}\n",
        sections.join("\n")
    )
}

fn render_ci_snippet(workflow: &str, actual_path: &Path) -> String {
    let actual_path = actual_path.to_string_lossy();
    let artifact_slug = slugify_workflow_name(workflow);
    format!(
        r#"# Starter CI snippet generated by `github-actionspec bootstrap`.
# Paste these steps into a workflow job after `actions/checkout`.

- name: Validate {workflow} contract
  id: actionspec
  uses: v4lproik/github-actionspec-rs@main
  with:
    workflow: {workflow}
    actual: {actual_path}
    report-file: /github/runner_temp/actionspec/current/validation-report.json
    dashboard-file: /github/runner_temp/actionspec/current/dashboard.md

- name: Upload validation artifacts
  if: ${{{{ always() }}}}
  uses: actions/upload-artifact@v4
  with:
    name: actionspec-{artifact_slug}-baseline
    path: |
      ${{{{ steps.actionspec.outputs.report-path }}}}
      ${{{{ steps.actionspec.outputs.dashboard-path }}}}
"#,
        artifact_slug = artifact_slug,
    )
}

pub fn bootstrap_repo_workflow(options: BootstrapOptions) -> Result<BootstrapResult, AppError> {
    let workflow_path = resolve_workflow_path(
        &options.repo_root,
        &options.workflows_dir,
        &options.workflow,
    );
    if !workflow_path.is_file() {
        return Err(AppError::MissingReadableFile(workflow_path));
    }

    let workflow_name = normalize_workflow_name(&options.workflow);
    let workflow_slug = slugify_workflow_name(&workflow_name);
    let declaration_path = options
        .repo_root
        .join(&options.declarations_dir)
        .join(&workflow_slug)
        .join("main.cue");
    let actual_path = options
        .repo_root
        .join(&options.fixtures_dir)
        .join(&workflow_slug)
        .join("baseline.json");
    let snippet_path = options
        .repo_root
        .join(&options.declarations_dir)
        .join(&workflow_slug)
        .join("bootstrap-ci-snippet.yml");

    assert_writable_output(&declaration_path, options.force)?;
    assert_writable_output(&actual_path, options.force)?;
    assert_writable_output(&snippet_path, options.force)?;

    let payload = match &options.actual {
        Some(actual_path) => load_actual_payload(actual_path, &workflow_name)?,
        None => synthesize_baseline_payload(&workflow_name, &workflow_path)?,
    };

    write_pretty_json_file(&payload, &actual_path)?;
    write_text_file(
        &declaration_path,
        &render_contract(&workflow_name, &payload),
    )?;
    let snippet_actual_path = actual_path
        .strip_prefix(&options.repo_root)
        .map(PathBuf::from)
        .unwrap_or_else(|_| actual_path.clone());
    write_text_file(
        &snippet_path,
        &render_ci_snippet(&workflow_name, &snippet_actual_path),
    )?;

    Ok(BootstrapResult {
        workflow: workflow_name,
        workflow_path,
        declaration_path,
        actual_path,
        snippet_path,
        seeded_from_actual: options.actual.is_some(),
    })
}

pub fn default_workflows_dir() -> PathBuf {
    PathBuf::from(DEFAULT_WORKFLOWS_DIR)
}

pub fn default_fixtures_dir() -> PathBuf {
    PathBuf::from(DEFAULT_FIXTURES_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn synthesizes_success_baseline_from_workflow_jobs() {
        let repo = tempdir().expect("temp dir should be created");
        let workflow_path = repo.path().join(".github/workflows/ci.yml");
        fs::create_dir_all(
            workflow_path
                .parent()
                .expect("workflow parent should exist"),
        )
        .expect("workflow dir should be created");
        fs::write(
            &workflow_path,
            r#"on:
  push:
jobs:
  lint:
    runs-on: ubuntu-latest
  build:
    runs-on: ubuntu-latest
"#,
        )
        .expect("workflow should be written");

        let payload =
            synthesize_baseline_payload("ci.yml", &workflow_path).expect("bootstrap should work");

        assert_eq!(payload.run.workflow, "ci.yml");
        assert_eq!(
            payload.run.jobs.keys().cloned().collect::<Vec<_>>(),
            vec!["build".to_owned(), "lint".to_owned()]
        );
        assert!(payload.run.jobs.values().all(|job| job.result == "success"));
    }

    #[test]
    fn render_contract_preserves_observed_outputs_matrix_and_steps() {
        let contract = render_contract(
            "ci.yml",
            &WorkflowRunEnvelope {
                run: WorkflowRunRecord {
                    workflow: "ci.yml".to_owned(),
                    ref_name: Some("main".to_owned()),
                    inputs: Some(BTreeMap::from([(
                        "run_ci".to_owned(),
                        Some("true".to_owned()),
                    )])),
                    jobs: BTreeMap::from([(
                        "build".to_owned(),
                        WorkflowJobRecord {
                            result: "success".to_owned(),
                            outputs: Some(BTreeMap::from([(
                                "artifact_name".to_owned(),
                                "build-linux-amd64".to_owned(),
                            )])),
                            matrix: Some(BTreeMap::from([
                                ("app".to_owned(), json!("build-ts-service")),
                                ("shard".to_owned(), json!(2)),
                            ])),
                            steps: Some(BTreeMap::from([(
                                "compile".to_owned(),
                                WorkflowStepRecord {
                                    conclusion: Some("success".to_owned()),
                                    outputs: Some(BTreeMap::from([(
                                        "digest".to_owned(),
                                        "sha256:abc123".to_owned(),
                                    )])),
                                },
                            )])),
                        },
                    )]),
                },
            },
        );

        assert!(contract.contains("workflow: \"ci.yml\""));
        assert!(contract.contains("ref: \"main\""));
        assert!(contract.contains("\"run_ci\": \"true\""));
        assert!(contract.contains("\"artifact_name\": \"build-linux-amd64\""));
        assert!(contract.contains("\"app\": \"build-ts-service\""));
        assert!(contract.contains("\"shard\": 2"));
        assert!(contract.contains("\"compile\": {"));
        assert!(contract.contains("\"digest\": \"sha256:abc123\""));
    }

    #[test]
    fn normalize_workflow_name_prefers_the_file_name() {
        assert_eq!(normalize_workflow_name("ci.yml"), "ci.yml");
        assert_eq!(
            normalize_workflow_name(".github/workflows/release.yml"),
            "release.yml"
        );
    }
}
