use std::fs;
use std::path::{Path, PathBuf};

use github_actionspec_rs::types::{ValidationIssueKind, ValidationStatus};
use github_actionspec_rs::validate::{
    validate_repo_workflow, ValidateRepoWorkflowOptions, ValidateRepoWorkflowResult,
};
use tempfile::tempdir;

fn declarations_dir() -> PathBuf {
    PathBuf::from(".github/actionspec")
}

fn write_file(path: &Path, contents: &str) {
    let parent = path
        .parent()
        .expect("test file should have a parent directory");
    fs::create_dir_all(parent).expect("test directories should be created");
    fs::write(path, contents).expect("test fixture should be written");
}

fn validate_repo(
    repo_root: &Path,
    actual_paths: Vec<PathBuf>,
) -> Result<ValidateRepoWorkflowResult, String> {
    validate_repo_workflow(ValidateRepoWorkflowOptions {
        repo_root: repo_root.to_path_buf(),
        workflow: Some("build.yml".to_owned()),
        actual_paths,
        declarations_dir: declarations_dir(),
        cwd: Some(repo_root.to_path_buf()),
        env: None,
    })
    .map_err(|error| error.to_string())
}

#[test]
fn real_cue_validates_repo_matrix_and_outputs() {
    let repo = tempdir().expect("temp dir should be created");
    let declaration = repo.path().join(".github/actionspec/build/main.cue");
    let actual = repo.path().join("actual.json");

    write_file(
        &declaration,
        r#"package actionspec

workflow: "build.yml"

run: #Declaration.run & {
  jobs: {
    build: {
      result: "success"
      matrix: {
        app: "build-ts-service"
        target: "linux-amd64"
      }
      outputs: {
        contract_build: run.jobs.build.matrix.app
        artifact_name: "\(run.jobs.build.matrix.app)-\(run.jobs.build.matrix.target)"
      }
    }
  }
}
"#,
    );
    write_file(
        &actual,
        r#"{
  "run": {
    "workflow": "build.yml",
    "jobs": {
      "build": {
        "result": "success",
        "matrix": {
          "app": "build-ts-service",
          "target": "linux-amd64"
        },
        "outputs": {
          "contract_build": "build-ts-service",
          "artifact_name": "build-ts-service-linux-amd64"
        }
      }
    }
  }
}
"#,
    );

    let result = validate_repo(repo.path(), vec![actual]).expect("validation should pass");

    assert_eq!(result.failed_count, 0);
    assert_eq!(result.report.actuals.len(), 1);
    assert_eq!(result.report.actuals[0].status, ValidationStatus::Passed);
    assert_eq!(result.report.actuals[0].workflow, "build.yml");
}

#[test]
fn real_cue_reports_cross_job_output_conflicts() {
    let repo = tempdir().expect("temp dir should be created");
    let declaration = repo.path().join(".github/actionspec/build/main.cue");
    let actual = repo.path().join("actual.json");

    write_file(
        &declaration,
        r#"package actionspec

workflow: "build.yml"

run: #Declaration.run & {
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
    );
    write_file(
        &actual,
        r#"{
  "run": {
    "workflow": "build.yml",
    "jobs": {
      "build": {
        "result": "success",
        "outputs": {
          "image_tag": "ghcr.io/acme/app:sha-123"
        }
      },
      "publish": {
        "result": "success",
        "outputs": {
          "published_tag": "ghcr.io/acme/app:sha-456"
        }
      }
    }
  }
}
"#,
    );

    let result =
        validate_repo(repo.path(), vec![actual.clone()]).expect("report should be produced");

    assert_eq!(result.failed_count, 1);
    assert_eq!(result.report.actuals.len(), 1);
    assert_eq!(result.report.actuals[0].status, ValidationStatus::Failed);
    let error = result.report.actuals[0]
        .error
        .as_deref()
        .expect("failed validation should report an error");
    assert!(error.contains(&actual.display().to_string()));
    assert!(error.contains("published_tag"));
    assert!(error.contains("ghcr.io/acme/app:sha-123"));
    assert!(error.contains("ghcr.io/acme/app:sha-456"));
    assert_eq!(result.report.actuals[0].issues.len(), 1);
    assert_eq!(
        result.report.actuals[0].issues[0].kind,
        ValidationIssueKind::ValueConflict
    );
    assert_eq!(
        result.report.actuals[0].issues[0].path.as_deref(),
        Some("run.jobs.publish.outputs.published_tag")
    );
}

#[test]
fn real_cue_reports_missing_required_runtime_fields() {
    let repo = tempdir().expect("temp dir should be created");
    let declaration = repo.path().join(".github/actionspec/build/main.cue");
    let actual = repo.path().join("actual.json");

    write_file(
        &declaration,
        r#"package actionspec

workflow: "build.yml"

run: #Declaration.run & {
  jobs: {
    build: {
      result: "success"
      outputs: {
        artifact_name: string
      }
    }
  }
}
"#,
    );
    write_file(
        &actual,
        r#"{
  "run": {
    "workflow": "build.yml",
    "jobs": {
      "build": {
        "result": "success"
      }
    }
  }
}
"#,
    );

    let result =
        validate_repo(repo.path(), vec![actual.clone()]).expect("report should be produced");

    assert_eq!(result.failed_count, 1);
    assert_eq!(result.report.actuals[0].status, ValidationStatus::Failed);
    assert!(result.report.actuals[0]
        .issues
        .iter()
        .any(
            |issue| issue.kind == ValidationIssueKind::ConstraintViolation
                && issue.path.as_deref() == Some("run.jobs.build.outputs.artifact_name")
        ));
}
