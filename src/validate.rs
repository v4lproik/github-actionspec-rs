use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use glob::glob;

use crate::contracts::{declaration_schema_path, workflow_schema_path};
use crate::discovery::find_declaration;
use crate::errors::AppError;
use crate::types::{
    ActualValidationReport, ValidationReport, ValidationStatus, WorkflowRunEnvelope,
};

#[derive(Debug, Clone)]
pub struct ValidateContractOptions {
    pub schema_paths: Vec<PathBuf>,
    pub contract_path: PathBuf,
    pub actual_paths: Vec<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct ValidateRepoWorkflowOptions {
    pub repo_root: PathBuf,
    pub workflow: Option<String>,
    pub actual_paths: Vec<PathBuf>,
    pub declarations_dir: PathBuf,
    pub report_file: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct ValidateRepoWorkflowResult {
    pub report: ValidationReport,
    pub failed_count: usize,
}

fn assert_readable(path: &Path) -> Result<(), AppError> {
    if !path.is_file() {
        return Err(AppError::MissingReadableFile(path.to_path_buf()));
    }

    Ok(())
}

fn collect_directory_actuals(path: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut actual_paths = fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry| entry.is_file())
        .filter(|entry| entry.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();

    actual_paths.sort();

    if actual_paths.is_empty() {
        return Err(AppError::NoActualFilesFound(path.to_path_buf()));
    }

    Ok(actual_paths)
}

fn has_glob_pattern(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.contains('*') || path.contains('?') || path.contains('[')
}

fn collect_glob_actuals(path: &Path) -> Result<Vec<PathBuf>, AppError> {
    let pattern = path.to_string_lossy().into_owned();
    let mut actual_paths = glob(&pattern)?
        .filter_map(Result::ok)
        .filter(|entry| entry.is_file())
        .collect::<Vec<_>>();

    actual_paths.sort();

    if actual_paths.is_empty() {
        return Err(AppError::NoActualGlobMatches(pattern));
    }

    Ok(actual_paths)
}

fn resolve_actual_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, AppError> {
    if paths.is_empty() {
        return Err(AppError::MissingActualPaths);
    }

    let mut resolved_paths = Vec::new();
    for path in paths {
        if path.is_dir() {
            resolved_paths.extend(collect_directory_actuals(path)?);
            continue;
        }

        if has_glob_pattern(path) {
            resolved_paths.extend(collect_glob_actuals(path)?);
            continue;
        }

        assert_readable(path)?;
        resolved_paths.push(path.to_path_buf());
    }

    if resolved_paths.is_empty() {
        return Err(AppError::MissingActualPaths);
    }

    Ok(resolved_paths)
}

fn infer_workflow_from_actuals(paths: &[PathBuf]) -> Result<String, AppError> {
    let mut workflows = BTreeSet::new();

    for path in paths {
        let contents = fs::read_to_string(path)?;
        let envelope: WorkflowRunEnvelope = serde_json::from_str(&contents)?;
        workflows.insert(envelope.run.workflow);
    }

    if workflows.len() == 1 {
        return Ok(workflows
            .into_iter()
            .next()
            .expect("single workflow should exist"));
    }

    Err(AppError::AmbiguousActualWorkflows(
        workflows.into_iter().collect::<Vec<_>>().join(", "),
    ))
}

fn read_workflow_run(path: &Path) -> Result<WorkflowRunEnvelope, AppError> {
    let contents = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

fn apply_env(command: &mut Command, env: &Option<HashMap<String, String>>) {
    if let Some(env_map) = env {
        // Tests and CI inject a controlled PATH for the fake `cue` binary, so the command must
        // not inherit the host process environment.
        command.env_clear();
        for (key, value) in env_map {
            command.env(key, value);
        }
    }
}

fn cue_command(
    env: &Option<HashMap<String, String>>,
    cwd: Option<&Path>,
    subcommand: &str,
) -> Command {
    let mut command = Command::new("cue");
    command.arg(subcommand);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    apply_env(&mut command, env);
    command
}

pub fn assert_cue_available(env: &Option<HashMap<String, String>>) -> Result<(), AppError> {
    let mut command = cue_command(env, None, "version");

    match command.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(AppError::CueVersionFailed),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(AppError::CueNotAvailable)
        }
        Err(error) => Err(AppError::Io(error)),
    }
}

fn run_cue_vet(
    schema_paths: &[PathBuf],
    contract_path: &Path,
    actual_path: &Path,
    cwd: Option<&Path>,
    env: &Option<HashMap<String, String>>,
) -> Result<(), String> {
    let mut command = cue_command(env, cwd, "vet");
    for schema_path in schema_paths {
        command.arg(schema_path);
    }
    command.arg(contract_path);
    command.arg(actual_path);

    match command.output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let code = output
                .status
                .code()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_owned());
            if stderr.is_empty() {
                Err(format!("cue vet failed with exit code {code}"))
            } else {
                Err(format!("cue vet failed with exit code {code}: {stderr}"))
            }
        }
        Err(error) => Err(error.to_string()),
    }
}

pub fn validate_contract(options: ValidateContractOptions) -> Result<(), AppError> {
    if options.schema_paths.is_empty() {
        return Err(AppError::MissingSchemaPaths);
    }

    for schema_path in &options.schema_paths {
        assert_readable(schema_path)?;
    }
    assert_readable(&options.contract_path)?;
    let actual_paths = resolve_actual_paths(&options.actual_paths)?;
    assert_cue_available(&options.env)?;

    for actual_path in &actual_paths {
        if let Err(error) = run_cue_vet(
            &options.schema_paths,
            &options.contract_path,
            actual_path,
            options.cwd.as_deref(),
            &options.env,
        ) {
            return Err(AppError::CueVetFailed(error));
        }
    }

    Ok(())
}

pub fn write_validation_report(report: &ValidationReport, path: &Path) -> Result<(), AppError> {
    fs::write(path, serde_json::to_string_pretty(report)?)?;
    Ok(())
}

pub fn validate_repo_workflow(
    options: ValidateRepoWorkflowOptions,
) -> Result<ValidateRepoWorkflowResult, AppError> {
    let ValidateRepoWorkflowOptions {
        repo_root,
        workflow,
        actual_paths,
        declarations_dir,
        report_file: _,
        cwd,
        env,
    } = options;

    let actual_paths = resolve_actual_paths(&actual_paths)?;
    let workflow = match workflow {
        Some(workflow) if !workflow.trim().is_empty() => workflow,
        _ => infer_workflow_from_actuals(&actual_paths)?,
    };

    let declaration = find_declaration(&repo_root, &workflow, Some(&declarations_dir))?;
    assert_cue_available(&env)?;

    let mut actual_reports = Vec::new();
    let mut failed = 0;

    for actual_path in actual_paths {
        let envelope = read_workflow_run(&actual_path)?;
        let matrix = envelope
            .run
            .jobs
            .values()
            .find_map(|job| job.matrix.clone());
        let jobs = envelope
            .run
            .jobs
            .into_iter()
            .map(|(job_name, job)| (job_name, job.result))
            .collect();
        let cue_result = run_cue_vet(
            &[workflow_schema_path(), declaration_schema_path()],
            &declaration.declaration_path,
            &actual_path,
            cwd.as_deref(),
            &env,
        );
        let (status, error) = match cue_result {
            Ok(()) => (ValidationStatus::Passed, None),
            Err(error) => {
                failed += 1;
                (ValidationStatus::Failed, Some(error))
            }
        };

        actual_reports.push(ActualValidationReport {
            actual_path,
            workflow: envelope.run.workflow,
            ref_name: envelope.run.ref_name,
            status,
            jobs,
            matrix,
            error,
        });
    }

    let report = ValidationReport {
        workflow,
        declaration_path: declaration.declaration_path,
        actuals: actual_reports,
    };

    Ok(ValidateRepoWorkflowResult {
        report,
        failed_count: failed,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn install_fake_cue(temp_root: &Path, script: &str) -> HashMap<String, String> {
        let bin_dir = temp_root.join("bin");
        fs::create_dir_all(&bin_dir).expect("bin dir should be created");
        let cue_path = bin_dir.join("cue");
        fs::write(&cue_path, script).expect("cue script should be written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&cue_path)
                .expect("metadata should exist")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&cue_path, permissions).expect("permissions should be updated");
        }

        let mut env: HashMap<String, String> = std::env::vars().collect();
        env.insert(
            "PATH".to_owned(),
            format!(
                "{}:{}",
                bin_dir.display(),
                env.get("PATH").cloned().unwrap_or_default()
            ),
        );
        env
    }

    fn write_validation_fixture(temp_root: &Path) -> (PathBuf, PathBuf) {
        let contract = temp_root.join("contract.cue");
        let actual = temp_root.join("actual.json");
        fs::write(&contract, "package actionspec\nrun: {}\n").expect("contract should be written");
        fs::write(&actual, "{}").expect("actual should be written");
        (contract, actual)
    }

    #[test]
    fn validate_contract_requires_schema_paths() {
        let error = validate_contract(ValidateContractOptions {
            schema_paths: vec![],
            contract_path: PathBuf::from("contract.cue"),
            actual_paths: vec![PathBuf::from("actual.json")],
            cwd: None,
            env: None,
        })
        .expect_err("validation should fail");

        assert!(error
            .to_string()
            .contains("At least one schema path is required."));
    }

    #[test]
    fn validate_contract_reports_missing_schema_file() {
        let temp = tempdir().expect("temp dir should be created");
        let (contract, actual) = write_validation_fixture(temp.path());

        let error = validate_contract(ValidateContractOptions {
            schema_paths: vec![temp.path().join("missing-schema.cue")],
            contract_path: contract,
            actual_paths: vec![actual],
            cwd: None,
            env: None,
        })
        .expect_err("validation should fail");

        assert!(error.to_string().contains("Missing readable file"));
        assert!(error.to_string().contains("missing-schema.cue"));
    }

    #[test]
    fn cue_availability_reports_missing_binary() {
        let env = HashMap::from([("PATH".to_owned(), "/definitely/missing".to_owned())]);
        let error = assert_cue_available(&Some(env)).expect_err("cue should be missing");

        assert!(error
            .to_string()
            .contains("The `cue` CLI is not installed or not available on PATH."));
    }

    #[test]
    fn cue_availability_reports_failed_version_command() {
        let temp = tempdir().expect("temp dir should be created");
        let env = install_fake_cue(
            temp.path(),
            "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  exit 2\nfi\nexit 1\n",
        );

        let error = assert_cue_available(&Some(env)).expect_err("cue version should fail");

        assert!(error
            .to_string()
            .contains("Failed to execute `cue version`."));
    }

    #[test]
    fn validate_repo_workflow_fails_when_declaration_is_missing() {
        let temp = tempdir().expect("temp dir should be created");
        let (_, actual) = write_validation_fixture(temp.path());

        let error = validate_repo_workflow(ValidateRepoWorkflowOptions {
            repo_root: temp.path().to_path_buf(),
            workflow: Some("missing.yml".to_owned()),
            actual_paths: vec![actual],
            declarations_dir: PathBuf::from(".github/actionspec"),
            report_file: None,
            cwd: None,
            env: None,
        })
        .expect_err("repo validation should fail");

        assert!(error
            .to_string()
            .contains("No declaration found for workflow \"missing.yml\""));
    }

    #[test]
    fn validate_contract_runs_once_per_actual_file() {
        let temp = tempdir().expect("temp dir should be created");
        let schema = temp.path().join("schema.cue");
        let contract = temp.path().join("contract.cue");
        let actual_dir = temp.path().join("actuals");
        let actual_one = actual_dir.join("actual-one.json");
        let actual_two = actual_dir.join("actual-two.json");
        fs::create_dir_all(&actual_dir).expect("actual dir should be created");
        fs::write(
            &schema,
            "package actionspec\n#WorkflowRun: {workflow: string, jobs: [string]: {result: string}}\n",
        )
        .expect("schema should be written");
        fs::write(
            &contract,
            "package actionspec\nrun: #WorkflowRun & {workflow: \"demo\", jobs: {build: {result: \"success\"}}}\n",
        )
        .expect("contract should be written");
        fs::write(
            &actual_one,
            "{\"run\":{\"workflow\":\"demo\",\"jobs\":{\"build\":{\"result\":\"success\"}}}}",
        )
        .expect("actual one should be written");
        fs::write(
            &actual_two,
            "{\"run\":{\"workflow\":\"demo\",\"jobs\":{\"build\":{\"result\":\"success\"}}}}",
        )
        .expect("actual two should be written");

        let env = install_fake_cue(
            temp.path(),
            "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"vet\" ]; then\n  json_count=0\n  shift\n  for arg in \"$@\"; do\n    case \"$arg\" in\n      *.json) json_count=$((json_count + 1));;\n    esac\n  done\n  if [ \"$json_count\" -ne 1 ]; then\n    exit 7\n  fi\n  exit 0\nfi\nexit 1\n",
        );

        let result = validate_contract(ValidateContractOptions {
            schema_paths: vec![schema],
            contract_path: contract,
            actual_paths: vec![actual_dir],
            cwd: Some(temp.path().to_path_buf()),
            env: Some(env),
        });

        assert!(result.is_ok());
    }

    #[test]
    fn validate_repo_workflow_infers_workflow_from_single_actual() {
        let temp = tempdir().expect("temp dir should be created");
        let declaration_dir = temp.path().join(".github/actionspec/ci");
        fs::create_dir_all(&declaration_dir).expect("declaration dir should be created");
        fs::write(
            declaration_dir.join("main.cue"),
            "package actionspec\n\nworkflow: \"ci.yml\"\n\nrun: #Declaration.run & {\n  workflow: workflow\n  jobs: {\n    sample: {\n      result: \"success\"\n    }\n  }\n}\n",
        )
        .expect("declaration should be written");

        let actual = temp.path().join("actual.json");
        fs::write(
            &actual,
            "{\"run\":{\"workflow\":\"ci.yml\",\"jobs\":{\"sample\":{\"result\":\"success\"}}}}",
        )
        .expect("actual should be written");

        let env = install_fake_cue(
            temp.path(),
            "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"vet\" ]; then\n  exit 0\nfi\nexit 1\n",
        );

        let result = validate_repo_workflow(ValidateRepoWorkflowOptions {
            repo_root: temp.path().to_path_buf(),
            workflow: None,
            actual_paths: vec![actual],
            declarations_dir: PathBuf::from(".github/actionspec"),
            report_file: None,
            cwd: None,
            env: Some(env),
        })
        .expect("validation should succeed");

        assert_eq!(result.failed_count, 0);
        assert_eq!(result.report.actuals.len(), 1);
    }

    #[test]
    fn validate_repo_workflow_errors_when_actuals_span_multiple_workflows() {
        let temp = tempdir().expect("temp dir should be created");
        let first = temp.path().join("first.json");
        let second = temp.path().join("second.json");
        fs::write(
            &first,
            "{\"run\":{\"workflow\":\"build.yml\",\"jobs\":{\"sample\":{\"result\":\"success\"}}}}",
        )
        .expect("first actual should be written");
        fs::write(
            &second,
            "{\"run\":{\"workflow\":\"deploy.yml\",\"jobs\":{\"sample\":{\"result\":\"success\"}}}}",
        )
        .expect("second actual should be written");

        let error = validate_repo_workflow(ValidateRepoWorkflowOptions {
            repo_root: temp.path().to_path_buf(),
            workflow: None,
            actual_paths: vec![first, second],
            declarations_dir: PathBuf::from(".github/actionspec"),
            report_file: None,
            cwd: None,
            env: None,
        })
        .expect_err("validation should fail");

        assert!(error
            .to_string()
            .contains("Could not infer a single workflow from the provided actual payloads"));
        assert!(error.to_string().contains("build.yml"));
        assert!(error.to_string().contains("deploy.yml"));
    }

    #[test]
    fn validate_repo_workflow_collects_failures_in_report() {
        let temp = tempdir().expect("temp dir should be created");
        let declaration_dir = temp.path().join(".github/actionspec/ci");
        fs::create_dir_all(&declaration_dir).expect("declaration dir should be created");
        fs::write(
            declaration_dir.join("main.cue"),
            "package actionspec\n\nworkflow: \"ci.yml\"\n\nrun: #Declaration.run & {\n  workflow: workflow\n  ref: \"main\"\n  jobs: {\n    build: {\n      result: \"success\"\n    }\n  }\n}\n",
        )
        .expect("declaration should be written");

        let passing = temp.path().join("passing.json");
        let failing = temp.path().join("failing.json");
        fs::write(
            &passing,
            "{\"run\":{\"workflow\":\"ci.yml\",\"ref\":\"main\",\"jobs\":{\"build\":{\"result\":\"success\"}}}}",
        )
        .expect("passing actual should be written");
        fs::write(
            &failing,
            "{\"run\":{\"workflow\":\"ci.yml\",\"ref\":\"main\",\"jobs\":{\"build\":{\"result\":\"skipped\"}}}}",
        )
        .expect("failing actual should be written");

        let env = install_fake_cue(
            temp.path(),
            "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"vet\" ]; then\n  last=\"\"\n  for arg in \"$@\"; do\n    last=\"$arg\"\n  done\n  case \"$last\" in\n    *passing.json) exit 0 ;;\n    *failing.json) echo \"build should not be skipped\" >&2; exit 9 ;;\n  esac\nfi\nexit 1\n",
        );

        let result = validate_repo_workflow(ValidateRepoWorkflowOptions {
            repo_root: temp.path().to_path_buf(),
            workflow: Some("ci.yml".to_owned()),
            actual_paths: vec![passing.clone(), failing.clone()],
            declarations_dir: PathBuf::from(".github/actionspec"),
            report_file: None,
            cwd: None,
            env: Some(env),
        })
        .expect("report should be produced");

        assert_eq!(result.failed_count, 1);
        assert_eq!(result.report.actuals.len(), 2);
        assert!(result
            .report
            .actuals
            .iter()
            .any(|actual| actual.actual_path == passing && actual.error.is_none()));
        assert!(result.report.actuals.iter().any(|actual| {
            actual.actual_path == failing
                && actual
                    .error
                    .as_ref()
                    .is_some_and(|error| error.contains("build should not be skipped"))
        }));
    }

    #[test]
    fn validate_contract_errors_when_actual_directory_has_no_json_files() {
        let temp = tempdir().expect("temp dir should be created");
        let contract = temp.path().join("contract.cue");
        let schema = temp.path().join("schema.cue");
        let empty_dir = temp.path().join("actuals");
        fs::create_dir_all(&empty_dir).expect("empty dir should be created");
        fs::write(
            &contract,
            "package actionspec\nrun: #WorkflowRun & {workflow: \"demo\", jobs: {build: {result: \"success\"}}}\n",
        )
        .expect("contract should be written");
        fs::write(
            &schema,
            "package actionspec\n#WorkflowRun: {workflow: string, jobs: [string]: {result: string}}\n",
        )
        .expect("schema should be written");

        let error = validate_contract(ValidateContractOptions {
            schema_paths: vec![schema],
            contract_path: contract,
            actual_paths: vec![empty_dir.clone()],
            cwd: None,
            env: Some(install_fake_cue(temp.path(), "success")),
        })
        .expect_err("validation should fail");

        assert!(error.to_string().contains(&empty_dir.display().to_string()));
    }

    #[test]
    fn validate_contract_expands_globbed_actual_files() {
        let temp = tempdir().expect("temp dir should be created");
        let schema = temp.path().join("schema.cue");
        let contract = temp.path().join("contract.cue");
        let actual_dir = temp.path().join("actuals");
        let actual_one = actual_dir.join("actual-one.json");
        let actual_two = actual_dir.join("actual-two.json");
        fs::create_dir_all(&actual_dir).expect("actual dir should be created");
        fs::write(
            &schema,
            "package actionspec\n#WorkflowRun: {workflow: string, jobs: [string]: {result: string}}\n",
        )
        .expect("schema should be written");
        fs::write(
            &contract,
            "package actionspec\nrun: #WorkflowRun & {workflow: \"demo\", jobs: {build: {result: \"success\"}}}\n",
        )
        .expect("contract should be written");
        fs::write(
            &actual_one,
            "{\"run\":{\"workflow\":\"demo\",\"jobs\":{\"build\":{\"result\":\"success\"}}}}",
        )
        .expect("actual one should be written");
        fs::write(
            &actual_two,
            "{\"run\":{\"workflow\":\"demo\",\"jobs\":{\"build\":{\"result\":\"success\"}}}}",
        )
        .expect("actual two should be written");

        let env = install_fake_cue(
            temp.path(),
            "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"vet\" ]; then\n  json_count=0\n  shift\n  for arg in \"$@\"; do\n    case \"$arg\" in\n      *.json) json_count=$((json_count + 1));;\n    esac\n  done\n  if [ \"$json_count\" -ne 1 ]; then\n    exit 7\n  fi\n  exit 0\nfi\nexit 1\n",
        );

        let result = validate_contract(ValidateContractOptions {
            schema_paths: vec![schema],
            contract_path: contract,
            actual_paths: vec![actual_dir.join("*.json")],
            cwd: Some(temp.path().to_path_buf()),
            env: Some(env),
        });

        assert!(result.is_ok(), "globbed actual files should validate");
    }

    #[test]
    fn validate_contract_errors_when_actual_glob_matches_nothing() {
        let temp = tempdir().expect("temp dir should be created");
        let contract = temp.path().join("contract.cue");
        let schema = temp.path().join("schema.cue");
        let missing_glob = temp.path().join("actuals").join("*.json");
        fs::write(&contract, "package actionspec\nrun: {}\n").expect("contract should be written");
        fs::write(
            &schema,
            "package actionspec\n#WorkflowRun: {workflow: string, jobs: [string]: {result: string}}\n",
        )
        .expect("schema should be written");

        let error = validate_contract(ValidateContractOptions {
            schema_paths: vec![schema],
            contract_path: contract,
            actual_paths: vec![missing_glob.clone()],
            cwd: None,
            env: None,
        })
        .expect_err("validation should fail");

        assert!(error
            .to_string()
            .contains("No files matched actual glob pattern"));
        assert!(error
            .to_string()
            .contains(missing_glob.to_string_lossy().as_ref()));
    }
}
