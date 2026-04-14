use std::collections::{btree_map::Entry, BTreeMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::errors::AppError;
use crate::fs_utils::{resolve_json_input_paths, write_pretty_json_file};
use crate::types::{WorkflowJobRecord, WorkflowRunEnvelope, WorkflowRunRecord, WorkflowStepRecord};

#[derive(Debug, Clone)]
pub struct CaptureWorkflowOptions {
    pub workflow: String,
    pub ref_name: Option<String>,
    pub inputs: Vec<String>,
    pub job_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct EmitFragmentOptions {
    pub job: String,
    pub result: String,
    pub outputs: Vec<String>,
    pub matrix: Vec<String>,
    pub step_conclusions: Vec<String>,
    pub step_outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CapturedJobFragment {
    #[serde(alias = "job_id")]
    pub job: String,
    pub result: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub matrix: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub steps: BTreeMap<String, WorkflowStepRecord>,
}

fn normalize_non_empty_capture_job(job: &str, path: &Path) -> Result<String, AppError> {
    let normalized = job.trim();
    if normalized.is_empty() {
        return Err(AppError::MissingCaptureJobName(path.to_path_buf()));
    }

    Ok(normalized.to_owned())
}

fn normalize_non_empty_emit_value(
    value: &str,
    missing_error: AppError,
) -> Result<String, AppError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(missing_error);
    }

    Ok(normalized.to_owned())
}

fn load_job_fragment(path: &Path) -> Result<CapturedJobFragment, AppError> {
    let fragment: CapturedJobFragment = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok(CapturedJobFragment {
        job: normalize_non_empty_capture_job(&fragment.job, path)?,
        ..fragment
    })
}

fn parse_inputs(inputs: &[String]) -> Result<Option<BTreeMap<String, Option<String>>>, AppError> {
    if inputs.is_empty() {
        return Ok(None);
    }

    let mut parsed = BTreeMap::new();
    for input in inputs {
        let (key, value) = input
            .split_once('=')
            .ok_or_else(|| AppError::InvalidCaptureInput(input.clone()))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(AppError::InvalidCaptureInput(input.clone()));
        }

        // Duplicate workflow inputs are almost always a configuration bug, so fail instead of
        // silently keeping the last value.
        let normalized_key = key.to_owned();
        let normalized_value = value.trim().to_owned();
        if let Some(previous_value) = parsed.insert(normalized_key.clone(), Some(normalized_value))
        {
            let first = format!(
                "{normalized_key}={}",
                previous_value.as_deref().unwrap_or("")
            );
            return Err(AppError::DuplicateCaptureInput {
                key: normalized_key,
                first,
                second: input.clone(),
            });
        }
    }

    Ok(Some(parsed))
}

fn normalize_workflow_name(workflow: String) -> Result<String, AppError> {
    let trimmed = workflow.trim();
    if trimmed.is_empty() {
        return Err(AppError::MissingCaptureWorkflow);
    }

    Ok(trimmed.to_owned())
}

fn normalize_ref_name(ref_name: Option<String>) -> Option<String> {
    ref_name.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

fn parse_key_value_assignment(
    field: &'static str,
    entry: &str,
    expected: &'static str,
    allow_empty_value: bool,
) -> Result<(String, String), AppError> {
    let (key, value) =
        entry
            .split_once('=')
            .ok_or_else(|| AppError::InvalidEmitFragmentArgument {
                field,
                value: entry.to_owned(),
                expected,
            })?;
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || (!allow_empty_value && value.is_empty()) {
        return Err(AppError::InvalidEmitFragmentArgument {
            field,
            value: entry.to_owned(),
            expected,
        });
    }

    Ok((key.to_owned(), value.to_owned()))
}

fn parse_fragment_outputs(entries: &[String]) -> Result<BTreeMap<String, String>, AppError> {
    let mut outputs = BTreeMap::new();
    let mut sources = BTreeMap::new();
    for entry in entries {
        let (key, value) = parse_key_value_assignment("output", entry, "KEY=VALUE", true)?;
        if let Some(first) = sources.insert(key.clone(), entry.clone()) {
            return Err(AppError::DuplicateEmitFragmentArgument {
                field: "output",
                key,
                first,
                second: entry.clone(),
            });
        }
        outputs.insert(key, value);
    }

    Ok(outputs)
}

fn parse_fragment_matrix(entries: &[String]) -> Result<BTreeMap<String, Value>, AppError> {
    let mut matrix = BTreeMap::new();
    let mut sources = BTreeMap::new();
    for entry in entries {
        let (key, value) = parse_key_value_assignment("matrix entry", entry, "KEY=VALUE", true)?;
        if let Some(first) = sources.insert(key.clone(), entry.clone()) {
            return Err(AppError::DuplicateEmitFragmentArgument {
                field: "matrix entry",
                key,
                first,
                second: entry.clone(),
            });
        }
        // Preserve native JSON scalars and arrays when possible so downstream contracts can
        // distinguish numbers and booleans from plain strings.
        let parsed = serde_json::from_str(&value).unwrap_or_else(|_| Value::String(value.clone()));
        matrix.insert(key, parsed);
    }

    Ok(matrix)
}

fn parse_step_conclusions(
    entries: &[String],
) -> Result<BTreeMap<String, WorkflowStepRecord>, AppError> {
    let mut steps = BTreeMap::new();
    let mut sources = BTreeMap::new();
    for entry in entries {
        let (step_id, conclusion) =
            parse_key_value_assignment("step conclusion", entry, "STEP_ID=CONCLUSION", false)?;
        if let Some(first) = sources.insert(step_id.clone(), entry.clone()) {
            return Err(AppError::DuplicateEmitFragmentArgument {
                field: "step conclusion",
                key: step_id,
                first,
                second: entry.clone(),
            });
        }
        steps.insert(
            step_id,
            WorkflowStepRecord {
                conclusion: Some(conclusion),
                outputs: None,
            },
        );
    }

    Ok(steps)
}

fn parse_step_output_assignment(entry: &str) -> Result<(String, String, String), AppError> {
    let (step_output, value) =
        parse_key_value_assignment("step output", entry, "STEP_ID.OUTPUT_NAME=VALUE", true)?;
    // Split on the last dot so step ids can still contain dots without breaking output parsing.
    let (step_id, output_name) =
        step_output
            .rsplit_once('.')
            .ok_or_else(|| AppError::InvalidEmitFragmentArgument {
                field: "step output",
                value: entry.to_owned(),
                expected: "STEP_ID.OUTPUT_NAME=VALUE",
            })?;
    let step_id = step_id.trim();
    let output_name = output_name.trim();
    if step_id.is_empty() || output_name.is_empty() {
        return Err(AppError::InvalidEmitFragmentArgument {
            field: "step output",
            value: entry.to_owned(),
            expected: "STEP_ID.OUTPUT_NAME=VALUE",
        });
    }

    Ok((step_id.to_owned(), output_name.to_owned(), value))
}

fn merge_step_outputs(
    steps: &mut BTreeMap<String, WorkflowStepRecord>,
    entries: &[String],
) -> Result<(), AppError> {
    let mut sources = BTreeMap::new();
    for entry in entries {
        let (step_id, output_name, value) = parse_step_output_assignment(entry)?;
        let qualified_key = format!("{step_id}.{output_name}");
        if let Some(first) = sources.insert(qualified_key.clone(), entry.clone()) {
            return Err(AppError::DuplicateEmitFragmentArgument {
                field: "step output",
                key: qualified_key,
                first,
                second: entry.clone(),
            });
        }
        let step = steps
            .entry(step_id.clone())
            .or_insert_with(|| WorkflowStepRecord {
                conclusion: None,
                outputs: None,
            });
        let outputs = step.outputs.get_or_insert_with(BTreeMap::new);
        outputs.insert(output_name, value);
    }

    Ok(())
}

fn fragment_job_record(fragment: CapturedJobFragment) -> WorkflowJobRecord {
    WorkflowJobRecord {
        result: fragment.result,
        outputs: (!fragment.outputs.is_empty()).then_some(fragment.outputs),
        matrix: (!fragment.matrix.is_empty()).then_some(fragment.matrix),
        steps: (!fragment.steps.is_empty()).then_some(fragment.steps),
    }
}

pub fn emit_job_fragment(options: EmitFragmentOptions) -> Result<CapturedJobFragment, AppError> {
    let mut steps = parse_step_conclusions(&options.step_conclusions)?;
    merge_step_outputs(&mut steps, &options.step_outputs)?;

    Ok(CapturedJobFragment {
        job: normalize_non_empty_emit_value(&options.job, AppError::MissingEmitFragmentJob)?,
        result: normalize_non_empty_emit_value(
            &options.result,
            AppError::MissingEmitFragmentResult,
        )?,
        outputs: parse_fragment_outputs(&options.outputs)?,
        matrix: parse_fragment_matrix(&options.matrix)?,
        steps,
    })
}

pub fn write_emitted_job_fragment(
    fragment: &CapturedJobFragment,
    output: &Path,
) -> Result<(), AppError> {
    write_pretty_json_file(fragment, output)
}

pub fn capture_workflow_run(
    options: CaptureWorkflowOptions,
) -> Result<WorkflowRunEnvelope, AppError> {
    let job_paths = resolve_json_input_paths(
        &options.job_files,
        || AppError::MissingCaptureJobFiles,
        AppError::NoCaptureJobFilesFound,
        AppError::NoCaptureJobGlobMatches,
    )?;
    let mut jobs = BTreeMap::new();
    let mut job_sources = BTreeMap::<String, PathBuf>::new();

    for path in job_paths {
        let fragment = load_job_fragment(&path)?;
        let job_name = fragment.job.clone();
        match job_sources.entry(job_name.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(path.clone());
                jobs.insert(job_name, fragment_job_record(fragment));
            }
            Entry::Occupied(entry) => {
                return Err(AppError::DuplicateCaptureJob {
                    job: job_name,
                    first: entry.get().clone(),
                    second: path,
                });
            }
        }
    }

    Ok(WorkflowRunEnvelope {
        run: WorkflowRunRecord {
            workflow: normalize_workflow_name(options.workflow)?,
            ref_name: normalize_ref_name(options.ref_name),
            inputs: parse_inputs(&options.inputs)?,
            jobs,
        },
    })
}

pub fn write_captured_workflow_run(
    envelope: &WorkflowRunEnvelope,
    output: &Path,
) -> Result<(), AppError> {
    write_pretty_json_file(envelope, output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_fragment(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fragment directory should be created");
        }
        fs::write(path, contents).expect("fragment should be written");
    }

    #[test]
    fn emits_fragment_with_outputs_matrix_and_steps() {
        let fragment = emit_job_fragment(EmitFragmentOptions {
            job: " build ".to_owned(),
            result: " success ".to_owned(),
            outputs: vec!["artifact=build-ts-service".to_owned(), "empty=".to_owned()],
            matrix: vec![
                "app=build-ts-service".to_owned(),
                "shard=2".to_owned(),
                "enabled=true".to_owned(),
            ],
            step_conclusions: vec!["compile=success".to_owned()],
            step_outputs: vec![
                "compile.digest=sha256:abc123".to_owned(),
                "publish.url=https://example.invalid/artifact".to_owned(),
            ],
        })
        .expect("fragment should be emitted");

        assert_eq!(fragment.job, "build");
        assert_eq!(fragment.result, "success");
        assert_eq!(
            fragment.outputs,
            BTreeMap::from([
                ("artifact".to_owned(), "build-ts-service".to_owned()),
                ("empty".to_owned(), String::new()),
            ])
        );
        assert_eq!(
            fragment.matrix.get("app"),
            Some(&Value::String("build-ts-service".to_owned()))
        );
        assert_eq!(fragment.matrix.get("shard"), Some(&Value::from(2)));
        assert_eq!(fragment.matrix.get("enabled"), Some(&Value::from(true)));
        assert_eq!(
            fragment.steps["compile"].conclusion.as_deref(),
            Some("success")
        );
        assert_eq!(
            fragment.steps["compile"]
                .outputs
                .as_ref()
                .and_then(|outputs| outputs.get("digest")),
            Some(&"sha256:abc123".to_owned())
        );
        assert_eq!(
            fragment.steps["publish"]
                .outputs
                .as_ref()
                .and_then(|outputs| outputs.get("url")),
            Some(&"https://example.invalid/artifact".to_owned())
        );
    }

    #[test]
    fn rejects_duplicate_emit_fragment_step_outputs() {
        let error = emit_job_fragment(EmitFragmentOptions {
            job: "build".to_owned(),
            result: "success".to_owned(),
            outputs: Vec::new(),
            matrix: Vec::new(),
            step_conclusions: Vec::new(),
            step_outputs: vec![
                "compile.digest=sha256:one".to_owned(),
                "compile.digest=sha256:two".to_owned(),
            ],
        })
        .expect_err("duplicate step outputs should fail");

        assert!(error
            .to_string()
            .contains("Duplicate emit-fragment step output `compile.digest`"));
    }

    #[test]
    fn rejects_blank_emit_fragment_job_name() {
        let error = emit_job_fragment(EmitFragmentOptions {
            job: "   ".to_owned(),
            result: "success".to_owned(),
            outputs: Vec::new(),
            matrix: Vec::new(),
            step_conclusions: Vec::new(),
            step_outputs: Vec::new(),
        })
        .expect_err("blank job names should fail");

        assert_eq!(
            error.to_string(),
            "emit-fragment job name must be non-empty."
        );
    }

    #[test]
    fn captures_workflow_run_from_job_fragments() {
        let temp = tempdir().expect("temp dir should be created");
        let fragments_dir = temp.path().join("fragments");
        write_fragment(
            &fragments_dir.join("build.json"),
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
            &fragments_dir.join("tests.json"),
            r#"{
  "job": "tests",
  "result": "success"
}"#,
        );

        let captured = capture_workflow_run(CaptureWorkflowOptions {
            workflow: "ci.yml".to_owned(),
            ref_name: Some("main".to_owned()),
            inputs: vec!["run_ci=true".to_owned(), "target=linux-amd64".to_owned()],
            job_files: vec![fragments_dir],
        })
        .expect("capture should succeed");

        assert_eq!(captured.run.workflow, "ci.yml");
        assert_eq!(captured.run.ref_name.as_deref(), Some("main"));
        assert_eq!(
            captured.run.inputs,
            Some(BTreeMap::from([
                ("run_ci".to_owned(), Some("true".to_owned())),
                ("target".to_owned(), Some("linux-amd64".to_owned())),
            ]))
        );
        assert_eq!(captured.run.jobs.len(), 2);
        assert_eq!(
            captured.run.jobs["build"].outputs,
            Some(BTreeMap::from([(
                "artifact_name".to_owned(),
                "build-ts-service-linux-amd64".to_owned(),
            )]))
        );
        assert_eq!(
            captured.run.jobs["build"]
                .steps
                .as_ref()
                .and_then(|steps| steps
                    .get("compile")
                    .and_then(|step| step.outputs.as_ref())
                    .and_then(|outputs| outputs.get("digest"))),
            Some(&"sha256:abc123".to_owned())
        );
        assert_eq!(captured.run.jobs["tests"].outputs, None);
    }

    #[test]
    fn rejects_duplicate_job_fragments() {
        let temp = tempdir().expect("temp dir should be created");
        let first = temp.path().join("build-a.json");
        let second = temp.path().join("build-b.json");
        write_fragment(&first, r#"{"job":"build","result":"success"}"#);
        write_fragment(&second, r#"{"job":"build","result":"failure"}"#);

        let error = capture_workflow_run(CaptureWorkflowOptions {
            workflow: "ci.yml".to_owned(),
            ref_name: None,
            inputs: Vec::new(),
            job_files: vec![first.clone(), second.clone()],
        })
        .expect_err("duplicate jobs should fail");

        assert!(error.to_string().contains("Duplicate captured job `build`"));
        assert!(error.to_string().contains(&first.display().to_string()));
        assert!(error.to_string().contains(&second.display().to_string()));
    }

    #[test]
    fn rejects_invalid_capture_inputs() {
        let temp = tempdir().expect("temp dir should be created");
        let fragment = temp.path().join("build.json");
        write_fragment(&fragment, r#"{"job":"build","result":"success"}"#);

        let error = capture_workflow_run(CaptureWorkflowOptions {
            workflow: "ci.yml".to_owned(),
            ref_name: None,
            inputs: vec!["missing-separator".to_owned()],
            job_files: vec![fragment],
        })
        .expect_err("invalid inputs should fail");

        assert!(error
            .to_string()
            .contains("Invalid capture input `missing-separator`"));
    }

    #[test]
    fn capture_trims_input_values_and_accepts_job_id_alias() {
        let temp = tempdir().expect("temp dir should be created");
        let fragment = temp.path().join("build.json");
        write_fragment(
            &fragment,
            r#"{
  "job_id": "build",
  "result": "success"
}"#,
        );

        let captured = capture_workflow_run(CaptureWorkflowOptions {
            workflow: "ci.yml".to_owned(),
            ref_name: Some(" main ".to_owned()),
            inputs: vec!["run_ci= true ".to_owned()],
            job_files: vec![fragment],
        })
        .expect("capture should succeed");

        assert_eq!(captured.run.ref_name.as_deref(), Some("main"));
        assert_eq!(
            captured.run.inputs,
            Some(BTreeMap::from([(
                "run_ci".to_owned(),
                Some("true".to_owned())
            )]))
        );
        assert!(captured.run.jobs.contains_key("build"));
    }

    #[test]
    fn rejects_duplicate_capture_inputs() {
        let temp = tempdir().expect("temp dir should be created");
        let fragment = temp.path().join("build.json");
        write_fragment(&fragment, r#"{"job":"build","result":"success"}"#);

        let error = capture_workflow_run(CaptureWorkflowOptions {
            workflow: "ci.yml".to_owned(),
            ref_name: None,
            inputs: vec!["run_ci=true".to_owned(), "run_ci=false".to_owned()],
            job_files: vec![fragment],
        })
        .expect_err("duplicate inputs should fail");

        assert!(error
            .to_string()
            .contains("Duplicate capture input `run_ci`"));
    }

    #[test]
    fn rejects_empty_capture_workflow_name() {
        let temp = tempdir().expect("temp dir should be created");
        let fragment = temp.path().join("build.json");
        write_fragment(&fragment, r#"{"job":"build","result":"success"}"#);

        let error = capture_workflow_run(CaptureWorkflowOptions {
            workflow: "   ".to_owned(),
            ref_name: None,
            inputs: Vec::new(),
            job_files: vec![fragment],
        })
        .expect_err("blank workflow names should fail");

        assert!(error
            .to_string()
            .contains("Capture workflow name must be non-empty."));
    }
}
