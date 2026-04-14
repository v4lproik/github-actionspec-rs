use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use glob::glob;
use serde::Deserialize;

use crate::errors::AppError;
use crate::types::{WorkflowJobRecord, WorkflowRunEnvelope, WorkflowRunRecord, WorkflowStepRecord};

#[derive(Debug, Clone)]
pub struct CaptureWorkflowOptions {
    pub workflow: String,
    pub ref_name: Option<String>,
    pub inputs: Vec<String>,
    pub job_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct JobCaptureFragment {
    #[serde(alias = "job_id")]
    job: String,
    result: String,
    #[serde(default)]
    outputs: BTreeMap<String, String>,
    #[serde(default)]
    matrix: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    steps: BTreeMap<String, WorkflowStepRecord>,
}

#[derive(Debug, Clone)]
struct LoadedJobFragment {
    path: PathBuf,
    fragment: JobCaptureFragment,
}

fn assert_json_file(path: &Path) -> Result<(), AppError> {
    if !path.is_file() {
        return Err(AppError::MissingReadableFile(path.to_path_buf()));
    }

    Ok(())
}

fn collect_fragment_directory(path: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut job_files = fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry| entry.is_file())
        .filter(|entry| entry.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();

    job_files.sort();

    if job_files.is_empty() {
        return Err(AppError::NoCaptureJobFilesFound(path.to_path_buf()));
    }

    Ok(job_files)
}

fn has_glob_pattern(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.contains('*') || path.contains('?') || path.contains('[')
}

fn collect_fragment_glob(path: &Path) -> Result<Vec<PathBuf>, AppError> {
    let pattern = path.to_string_lossy().into_owned();
    let mut job_files = glob(&pattern)?
        .filter_map(Result::ok)
        .filter(|entry| entry.is_file())
        .collect::<Vec<_>>();

    job_files.sort();

    if job_files.is_empty() {
        return Err(AppError::NoCaptureJobGlobMatches(pattern));
    }

    Ok(job_files)
}

fn resolve_job_fragment_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, AppError> {
    if paths.is_empty() {
        return Err(AppError::MissingCaptureJobFiles);
    }

    let mut resolved_paths = BTreeSet::new();
    for path in paths {
        if path.is_dir() {
            resolved_paths.extend(collect_fragment_directory(path)?);
            continue;
        }

        if has_glob_pattern(path) {
            resolved_paths.extend(collect_fragment_glob(path)?);
            continue;
        }

        assert_json_file(path)?;
        resolved_paths.insert(path.to_path_buf());
    }

    if resolved_paths.is_empty() {
        return Err(AppError::MissingCaptureJobFiles);
    }

    Ok(resolved_paths.into_iter().collect())
}

fn load_job_fragment(path: &Path) -> Result<LoadedJobFragment, AppError> {
    let fragment: JobCaptureFragment = serde_json::from_str(&fs::read_to_string(path)?)?;
    let job_name = fragment.job.trim();
    if job_name.is_empty() {
        return Err(AppError::MissingCaptureJobName(path.to_path_buf()));
    }

    Ok(LoadedJobFragment {
        path: path.to_path_buf(),
        fragment: JobCaptureFragment {
            job: job_name.to_owned(),
            ..fragment
        },
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

        parsed.insert(key.to_owned(), Some(value.to_owned()));
    }

    Ok(Some(parsed))
}

fn normalize_ref_name(ref_name: Option<String>) -> Option<String> {
    ref_name.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

fn fragment_job_record(fragment: JobCaptureFragment) -> WorkflowJobRecord {
    WorkflowJobRecord {
        result: fragment.result,
        outputs: (!fragment.outputs.is_empty()).then_some(fragment.outputs),
        matrix: (!fragment.matrix.is_empty()).then_some(fragment.matrix),
        steps: (!fragment.steps.is_empty()).then_some(fragment.steps),
    }
}

pub fn capture_workflow_run(
    options: CaptureWorkflowOptions,
) -> Result<WorkflowRunEnvelope, AppError> {
    let job_paths = resolve_job_fragment_paths(&options.job_files)?;
    let mut jobs = BTreeMap::new();
    let mut job_sources = BTreeMap::<String, PathBuf>::new();

    for path in job_paths {
        let loaded = load_job_fragment(&path)?;
        let job_name = loaded.fragment.job.clone();
        match job_sources.entry(job_name.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(loaded.path);
                jobs.insert(job_name, fragment_job_record(loaded.fragment));
            }
            Entry::Occupied(entry) => {
                return Err(AppError::DuplicateCaptureJob {
                    job: job_name,
                    first: entry.get().clone(),
                    second: loaded.path,
                });
            }
        }
    }

    Ok(WorkflowRunEnvelope {
        run: WorkflowRunRecord {
            workflow: options.workflow,
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
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, serde_json::to_string_pretty(envelope)?)?;
    Ok(())
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
}
