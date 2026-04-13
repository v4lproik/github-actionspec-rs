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

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Regex(#[from] regex::Error),
}
