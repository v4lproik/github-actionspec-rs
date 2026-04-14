use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("No declaration found for workflow \"{workflow}\" under {root}")]
    DeclarationNotFound { workflow: String, root: String },

    #[error("Missing top-level workflow declaration in {0}")]
    MissingWorkflowDeclaration(String),

    #[error("The `cue` CLI is not installed or not available on PATH.")]
    CueNotAvailable,

    #[error("Failed to execute `cue version`.")]
    CueVersionFailed,

    #[error("cue vet failed with exit code {0}")]
    CueVetFailed(String),

    #[error("At least one schema path is required.")]
    MissingSchemaPaths,

    #[error("At least one actual path is required.")]
    MissingActualPaths,

    #[error("Missing readable file: {0}")]
    MissingReadableFile(PathBuf),

    #[error("No readable JSON files were found under directory: {0}")]
    NoActualFilesFound(PathBuf),

    #[error(
        "Could not infer a single workflow from the provided actual payloads. Pass --workflow explicitly. Found workflows: {0}"
    )]
    AmbiguousActualWorkflows(String),

    #[error("No files matched actual glob pattern: {0}")]
    NoActualGlobMatches(String),

    #[error("At least one job fragment path is required.")]
    MissingCaptureJobFiles,

    #[error("No readable JSON job fragment files were found under directory: {0}")]
    NoCaptureJobFilesFound(PathBuf),

    #[error("No files matched job fragment glob pattern: {0}")]
    NoCaptureJobGlobMatches(String),

    #[error("Job fragment is missing a non-empty `job` field: {0}")]
    MissingCaptureJobName(PathBuf),

    #[error("Duplicate captured job `{job}` found in {first} and {second}")]
    DuplicateCaptureJob {
        job: String,
        first: PathBuf,
        second: PathBuf,
    },

    #[error("Invalid capture input `{0}`. Expected KEY=VALUE.")]
    InvalidCaptureInput(String),

    #[error("Reusable workflow validation failed with {failed} issue(s).\n{details}")]
    WorkflowCallerValidationFailures { failed: usize, details: String },

    #[error("Validation failed for {failed} of {total} payloads.\n{details}")]
    ValidationFailures {
        failed: usize,
        total: usize,
        details: String,
    },

    #[error(transparent)]
    GlobPattern(#[from] glob::PatternError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error(transparent)]
    Regex(#[from] regex::Error),
}
