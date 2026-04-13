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

    #[error(
        "Validation failed for contract {contract_path} against payload {actual_path} (cue vet exit code {exit_code})\n{details}"
    )]
    CueVetFailed {
        exit_code: String,
        contract_path: PathBuf,
        actual_path: PathBuf,
        details: String,
    },

    #[error("At least one schema path is required.")]
    MissingSchemaPaths,

    #[error("At least one actual path is required.")]
    MissingActualPaths,

    #[error("Missing readable file: {0}")]
    MissingReadableFile(PathBuf),

    #[error("No readable JSON files were found under directory: {0}")]
    NoActualFilesFound(PathBuf),

    #[error("No files matched actual glob pattern: {0}")]
    NoActualGlobMatches(String),

    #[error(
        "Validation failed for workflow \"{workflow}\" using declaration {declaration_path} against payload {actual_path} (cue vet exit code {exit_code})\n{details}"
    )]
    RepoValidationFailed {
        workflow: String,
        declaration_path: PathBuf,
        actual_path: PathBuf,
        exit_code: String,
        details: String,
    },

    #[error(transparent)]
    GlobPattern(#[from] glob::PatternError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Regex(#[from] regex::Error),
}
