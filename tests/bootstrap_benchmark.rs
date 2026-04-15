use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use github_actionspec_rs::bootstrap::{bootstrap_repo_workflow, BootstrapOptions};
use tempfile::tempdir;

fn write_large_workflow(repo_root: &std::path::Path, workflow_name: &str, job_count: usize) {
    let workflow_path = repo_root.join(".github/workflows").join(workflow_name);
    fs::create_dir_all(
        workflow_path
            .parent()
            .expect("workflow parent should exist"),
    )
    .expect("workflow dir should be created");

    let jobs = (0..job_count)
        .map(|index| {
            format!(
                r#"  job-{index:03}:
    needs: [{needs}]
    if: ${{{{ always() }}}}
    strategy:
      matrix:
        shard: [1, 2, 3]
        target: [linux-amd64, linux-arm64]
    runs-on: ubuntu-latest
    steps:
      - id: prepare
        run: echo prepare
      - id: publish
        run: echo publish
"#,
                needs = if index == 0 {
                    String::new()
                } else {
                    format!("job-{:03}", index - 1)
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(
        workflow_path,
        format!(
            r#"on:
  pull_request:
  workflow_dispatch:
jobs:
{jobs}
"#
        ),
    )
    .expect("workflow should be written");
}

#[test]
#[ignore = "benchmark"]
fn benchmark_bootstrap_large_workflow() {
    let repo = tempdir().expect("temp dir should be created");
    write_large_workflow(repo.path(), "bench-bootstrap.yml", 120);

    let iterations = 25usize;
    let start = Instant::now();

    for _ in 0..iterations {
        let result = bootstrap_repo_workflow(BootstrapOptions {
            repo_root: repo.path().to_path_buf(),
            workflow: "bench-bootstrap.yml".to_owned(),
            actual: None,
            declarations_dir: PathBuf::from(".github/actionspec"),
            workflows_dir: PathBuf::from(".github/workflows"),
            fixtures_dir: PathBuf::from("tests/fixtures"),
            force: true,
        })
        .expect("bootstrap should succeed");
        black_box(result);
    }

    let elapsed = start.elapsed();
    let per_iteration = elapsed / iterations as u32;

    println!(
        "bootstrap benchmark: workflow=bench-bootstrap.yml jobs=120 iterations={iterations} total_ms={} per_iteration_ms={}",
        elapsed.as_millis(),
        per_iteration.as_millis()
    );
}
