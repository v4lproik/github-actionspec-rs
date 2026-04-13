mod support;

use github_actionspec_rs::discovery::{discover_declarations, find_declaration};
use tempfile::tempdir;

#[test]
fn discovers_workflow_declarations() {
    let repo = tempdir().unwrap();
    support::write_declaration(repo.path(), ".github/actionspec/ci/main.cue", "ci.yml");
    support::write_declaration(
        repo.path(),
        ".github/actionspec/release/default.cue",
        "release.yml",
    );

    let declarations =
        discover_declarations(repo.path(), std::path::Path::new(".github/actionspec")).unwrap();
    assert_eq!(declarations.len(), 2);
    assert_eq!(declarations[0].workflow, "ci.yml");
    assert_eq!(
        declarations[0].relative_path,
        std::path::PathBuf::from(".github/actionspec/ci/main.cue")
    );
    assert_eq!(declarations[1].workflow, "release.yml");
}

#[test]
fn finds_specific_declaration() {
    let repo = tempdir().unwrap();
    support::write_declaration(repo.path(), ".github/actionspec/ci/main.cue", "ci.yml");

    let declaration = find_declaration(
        repo.path(),
        "ci.yml",
        Some(std::path::Path::new(".github/actionspec")),
    )
    .unwrap();

    assert_eq!(declaration.workflow, "ci.yml");
}
