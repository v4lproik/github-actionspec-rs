mod support;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn validates_repo_contract_through_cli() {
    let repo = tempdir().unwrap();
    support::write_declaration(
        repo.path(),
        ".github/actionspec/build-infrastructure/staging.cue",
        "build-infrastructure.yml",
    );
    let actual = repo.path().join("actual.json");
    support::write_actual(&actual, "build-infrastructure.yml");
    let env = support::install_fake_cue(&repo, "success");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .envs(env)
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--workflow")
        .arg("build-infrastructure.yml")
        .arg("--actual")
        .arg(&actual);

    command.assert().success();
}

#[test]
fn discovers_repo_contracts_through_cli() {
    let repo = tempdir().unwrap();
    support::write_declaration(
        repo.path(),
        ".github/actionspec/test-e2e/default.cue",
        "test-e2e.yml",
    );

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command.arg("discover").arg("--repo").arg(repo.path());

    command
        .assert()
        .success()
        .stdout(predicate::str::contains("test-e2e.yml"))
        .stdout(predicate::str::contains(
            ".github/actionspec/test-e2e/default.cue",
        ));
}
