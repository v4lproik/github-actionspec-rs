mod support;

use github_actionspec_rs::discovery::{discover_declarations, find_declaration};
use tempfile::tempdir;

#[test]
fn discovers_workflow_declarations() {
    let repo = tempdir().unwrap();
    support::write_declaration(
        repo.path(),
        ".github/actionspec/build-infrastructure/staging.cue",
        "build-infrastructure.yml",
    );
    support::write_declaration(
        repo.path(),
        ".github/actionspec/test-e2e/default.cue",
        "test-e2e.yml",
    );

    let declarations = discover_declarations(repo.path(), std::path::Path::new(".github/actionspec")).unwrap();
    assert_eq!(declarations.len(), 2);
    assert_eq!(declarations[0].workflow, "build-infrastructure.yml");
    assert_eq!(
        declarations[0].relative_path,
        std::path::PathBuf::from(".github/actionspec/build-infrastructure/staging.cue")
    );
    assert_eq!(declarations[1].workflow, "test-e2e.yml");
}

#[test]
fn finds_specific_declaration() {
    let repo = tempdir().unwrap();
    support::write_declaration(
        repo.path(),
        ".github/actionspec/build-infrastructure/staging.cue",
        "build-infrastructure.yml",
    );

    let declaration = find_declaration(
        repo.path(),
        "build-infrastructure.yml",
        Some(std::path::Path::new(".github/actionspec")),
    )
    .unwrap();

    assert_eq!(declaration.workflow, "build-infrastructure.yml");
}
