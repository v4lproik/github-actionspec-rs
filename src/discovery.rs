use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use walkdir::WalkDir;

use crate::contracts::{resolve_declarations_dir, DEFAULT_DECLARATIONS_DIR};
use crate::errors::AppError;
use crate::types::WorkflowDeclaration;

fn workflow_regex() -> Result<Regex, AppError> {
    static WORKFLOW_REGEX: OnceLock<Result<Regex, regex::Error>> = OnceLock::new();

    WORKFLOW_REGEX
        .get_or_init(|| Regex::new(r#"(?m)^workflow:\s*\"([^\"\n]+)\"\s*$"#))
        .clone()
        .map_err(AppError::from)
}

fn extract_workflow(contents: &str, file_path: &Path) -> Result<String, AppError> {
    let regex = workflow_regex()?;
    let captures = regex
        .captures(contents)
        .ok_or_else(|| AppError::MissingWorkflowDeclaration(file_path.display().to_string()))?;

    Ok(captures
        .get(1)
        .expect("workflow capture should exist")
        .as_str()
        .to_owned())
}

pub fn discover_declarations(
    repo_root: &Path,
    declarations_dir: &Path,
) -> Result<Vec<WorkflowDeclaration>, AppError> {
    let root = resolve_declarations_dir(repo_root, declarations_dir);
    let mut declarations = Vec::new();

    for entry in WalkDir::new(&root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("cue")
        {
            continue;
        }

        let contents = fs::read_to_string(path)?;
        let workflow = extract_workflow(&contents, path)?;
        declarations.push(WorkflowDeclaration {
            workflow,
            declaration_path: path.to_path_buf(),
            // Keep a repo-relative path in the discovery payload so callers can surface stable
            // file references without depending on the machine-local checkout root.
            relative_path: path.strip_prefix(repo_root).unwrap_or(path).to_path_buf(),
        });
    }

    declarations.sort_by(|left, right| {
        left.workflow
            .cmp(&right.workflow)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });

    Ok(declarations)
}

pub fn find_declaration(
    repo_root: &Path,
    workflow: &str,
    declarations_dir: Option<&Path>,
) -> Result<WorkflowDeclaration, AppError> {
    let declarations_dir = declarations_dir.unwrap_or_else(|| Path::new(DEFAULT_DECLARATIONS_DIR));
    let declarations = discover_declarations(repo_root, declarations_dir)?;
    declarations
        .into_iter()
        .find(|entry| entry.workflow == workflow)
        .ok_or_else(|| AppError::DeclarationNotFound {
            workflow: workflow.to_owned(),
            root: resolve_declarations_dir(repo_root, declarations_dir)
                .display()
                .to_string(),
        })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn extracts_workflow_from_a_valid_declaration() {
        let workflow = extract_workflow(
            "package actionspec\n\nworkflow: \"ci.yml\"\n",
            Path::new("contract.cue"),
        )
        .expect("workflow should parse");

        assert_eq!(workflow, "ci.yml");
    }

    #[test]
    fn errors_when_workflow_is_missing_from_declaration() {
        let error = extract_workflow("package actionspec\n\nrun: {}\n", Path::new("broken.cue"))
            .expect_err("workflow extraction should fail");

        assert!(error
            .to_string()
            .contains("Missing top-level workflow declaration in broken.cue"));
    }

    #[test]
    fn discover_returns_empty_when_declaration_directory_exists_but_has_no_cue_files() {
        let repo = tempdir().expect("temp dir should be created");
        let declarations_dir = repo.path().join(".github/actionspec");
        fs::create_dir_all(&declarations_dir).expect("dir should be created");

        let declarations = discover_declarations(repo.path(), Path::new(".github/actionspec"))
            .expect("discover should succeed");

        assert!(declarations.is_empty());
    }
}
