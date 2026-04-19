use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowDeclaration {
    pub workflow: String,
    pub declaration_path: PathBuf,
    pub relative_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowRunEnvelope {
    pub run: WorkflowRunRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowRunRecord {
    pub workflow: String,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<BTreeMap<String, Option<String>>>,
    pub jobs: BTreeMap<String, WorkflowJobRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowJobRecord {
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<BTreeMap<String, WorkflowStepRecord>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowStepRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ValidationIssueKind {
    ValueConflict,
    UnexpectedField,
    MissingField,
    ConstraintViolation,
    CueError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValidationIssue {
    pub kind: ValidationIssueKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
}

impl ValidationIssueKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ValueConflict => "conflict",
            Self::UnexpectedField => "unexpected field",
            Self::MissingField => "missing field",
            Self::ConstraintViolation => "constraint",
            Self::CueError => "cue error",
        }
    }
}

impl ValidationIssue {
    #[must_use]
    pub fn summary_label(&self) -> String {
        match &self.path {
            Some(path) => format!("{}: {path}", self.kind.label()),
            None => self.kind.label().to_owned(),
        }
    }

    #[must_use]
    pub fn delta_label(&self) -> String {
        match &self.path {
            Some(path) => format!("{}@{path}", self.kind.label()),
            None => self.kind.label().to_owned(),
        }
    }

    #[must_use]
    pub fn detail_label(&self) -> String {
        match (&self.path, &self.expected, &self.actual) {
            (Some(path), Some(expected), Some(actual)) => {
                format!("{path}: {} ({expected} -> {actual})", self.message)
            }
            (Some(path), _, _) => format!("{path}: {}", self.message),
            _ => self.message.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActualValidationReport {
    pub actual_path: PathBuf,
    pub workflow: String,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    pub status: ValidationStatus,
    pub jobs: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<BTreeMap<String, BTreeMap<String, String>>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<ValidationIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationReport {
    pub workflow: String,
    pub declaration_path: PathBuf,
    pub actuals: Vec<ActualValidationReport>,
}
