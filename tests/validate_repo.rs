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
fn infers_workflow_from_single_actual_through_cli() {
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
        .arg("--actual")
        .arg(&actual);

    command.assert().success();
}

#[test]
fn errors_when_workflow_cannot_be_inferred_from_mixed_actuals() {
    let repo = tempdir().unwrap();
    let first = repo.path().join("first.json");
    let second = repo.path().join("second.json");
    support::write_actual(&first, "build.yml");
    support::write_actual(&second, "deploy.yml");

    let mut command = Command::cargo_bin("github-actionspec").unwrap();
    command
        .arg("validate-repo")
        .arg("--repo")
        .arg(repo.path())
        .arg("--actual")
        .arg(&first)
        .arg("--actual")
        .arg(&second);

    command
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Could not infer a single workflow from the provided actual payloads",
        ))
        .stderr(predicate::str::contains("build.yml"))
        .stderr(predicate::str::contains("deploy.yml"));
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
