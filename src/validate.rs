use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::contracts::{declaration_schema_path, workflow_schema_path};
use crate::discovery::find_declaration;
use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct ValidateContractOptions {
    pub schema_paths: Vec<PathBuf>,
    pub contract_path: PathBuf,
    pub actual_path: PathBuf,
    pub cwd: Option<PathBuf>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct ValidateRepoWorkflowOptions {
    pub repo_root: PathBuf,
    pub workflow: String,
    pub actual_path: PathBuf,
    pub declarations_dir: PathBuf,
    pub cwd: Option<PathBuf>,
    pub env: Option<HashMap<String, String>>,
}

fn assert_readable(path: &Path) -> Result<(), AppError> {
    if !path.is_file() {
        return Err(AppError::MissingReadableFile(path.to_path_buf()));
    }

    Ok(())
}

fn apply_env(command: &mut Command, env: &Option<HashMap<String, String>>) {
    if let Some(env_map) = env {
        command.env_clear();
        for (key, value) in env_map {
            command.env(key, value);
        }
    }
}

pub fn assert_cue_available(env: &Option<HashMap<String, String>>) -> Result<(), AppError> {
    let mut command = Command::new("cue");
    command.arg("version");
    apply_env(&mut command, env);

    match command.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(AppError::CueVersionFailed),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(AppError::CueNotAvailable),
        Err(error) => Err(AppError::Io(error)),
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
    assert_readable(&options.actual_path)?;
    assert_cue_available(&options.env)?;

    let mut command = Command::new("cue");
    command.arg("vet");
    for schema_path in &options.schema_paths {
        command.arg(schema_path);
    }
    command.arg(&options.contract_path);
    command.arg(&options.actual_path);
    if let Some(cwd) = &options.cwd {
        command.current_dir(cwd);
    }
    apply_env(&mut command, &options.env);

    let status = command.status()?;
    if status.success() {
        return Ok(());
    }

    Err(AppError::CueVetFailed(
        status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".to_owned()),
    ))
}

pub fn validate_repo_workflow(options: ValidateRepoWorkflowOptions) -> Result<(), AppError> {
    let declaration = find_declaration(
        &options.repo_root,
        &options.workflow,
        Some(&options.declarations_dir),
    )?;

    validate_contract(ValidateContractOptions {
        schema_paths: vec![workflow_schema_path(), declaration_schema_path()],
        contract_path: declaration.declaration_path,
        actual_path: options.actual_path,
        cwd: options.cwd,
        env: options.env,
    })
}
