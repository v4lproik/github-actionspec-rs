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
fn validates_repo_contract_directory_through_cli() {
    let repo = tempdir().unwrap();
    support::write_declaration(
        repo.path(),
        ".github/actionspec/build-infrastructure/staging.cue",
        "build-infrastructure.yml",
    );
    let actual_dir = repo.path().join("actuals");
    std::fs::create_dir_all(&actual_dir).unwrap();
    let actual_one = actual_dir.join("actual-one.json");
    let actual_two = actual_dir.join("actual-two.json");
    support::write_actual(&actual_one, "build-infrastructure.yml");
    support::write_actual(&actual_two, "build-infrastructure.yml");
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
        .arg(&actual_dir);

    command.assert().success();
}

#[test]
fn validates_repo_contract_globbed_actuals_through_cli() {
    let repo = tempdir().unwrap();
    support::write_declaration(
        repo.path(),
        ".github/actionspec/build-infrastructure/staging.cue",
        "build-infrastructure.yml",
    );
    let actual_dir = repo.path().join("actuals");
    std::fs::create_dir_all(&actual_dir).unwrap();
    let actual_one = actual_dir.join("actual-one.json");
    let actual_two = actual_dir.join("actual-two.json");
    support::write_actual(&actual_one, "build-infrastructure.yml");
    support::write_actual(&actual_two, "build-infrastructure.yml");
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
        .arg(actual_dir.join("*.json"));

    command.assert().success();
}

#[test]
fn reports_repo_validation_context_on_failure() {
    let repo = tempdir().unwrap();
    support::write_declaration(
        repo.path(),
        ".github/actionspec/build-infrastructure/staging.cue",
        "build-infrastructure.yml",
    );
    let actual = repo.path().join("actual.json");
    support::write_actual(&actual, "build-infrastructure.yml");
    let actual_display = actual.display().to_string();
    let declaration_display = repo
        .path()
        .join(".github/actionspec/build-infrastructure/staging.cue")
        .display()
        .to_string();
    let env = support::install_fake_cue(&repo, "failure");

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

    command
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Validation failed for workflow \"build-infrastructure.yml\"",
        ))
        .stderr(predicate::str::contains(&declaration_display))
        .stderr(predicate::str::contains(&actual_display))
        .stderr(predicate::str::contains("cue vet exit code 9"));
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
