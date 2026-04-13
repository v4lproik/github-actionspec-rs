use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use tempfile::tempdir;

fn write_executable(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn action_entrypoint_writes_dashboard_outputs_and_updates_pr_comment() {
    let temp = tempdir().unwrap();
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    write_executable(
        &bin_dir.join("github-actionspec"),
        r#"#!/bin/sh
set -eu
cmd="$1"
shift
if [ "$cmd" = "validate-repo" ]; then
  report_file=""
  prev=""
  for arg in "$@"; do
    if [ "$prev" = "--report-file" ]; then
      report_file="$arg"
    fi
    prev="$arg"
  done
  mkdir -p "$(dirname "$report_file")"
  cat > "$report_file" <<'EOF'
{
  "workflow": "ci.yml",
  "declaration_path": ".github/actionspec/ci/main.cue",
  "actuals": [
    {
      "actual_path": "tests/fixtures/ci/ci-main-success.json",
      "workflow": "ci.yml",
      "ref": "main",
      "status": "passed",
      "jobs": {
        "build": "success",
        "pages": "skipped"
      }
    }
  ]
}
EOF
  exit 0
fi
if [ "$cmd" = "dashboard" ]; then
  output=""
  prev=""
  for arg in "$@"; do
    if [ "$prev" = "--output" ]; then
      output="$arg"
    fi
    prev="$arg"
  done
  mkdir -p "$(dirname "$output")"
  cat > "$output" <<'EOF'
## Validation Matrix

| Payload | Ref | Status | build | pages | Delta |
| --- | --- | --- | --- | --- | --- |
| `tests/fixtures/ci/ci-main-success.json` | `main` | `Passed` | `success` | `skipped` | same |
EOF
  exit 0
fi
exit 1
"#,
    );

    write_executable(
        &bin_dir.join("jq"),
        r#"#!/bin/sh
set -eu
case "$*" in
  *".workflow"*)
    echo "ci.yml"
    ;;
  *".declaration_path"*)
    echo ".github/actionspec/ci/main.cue"
    ;;
  *".actuals | length"*)
    echo "1"
    ;;
  *'.actuals[] | select(.status == $status_name)] | length'*)
    case "$*" in
      *"passed"*)
        echo "1"
        ;;
      *"failed"*)
        echo "0"
        ;;
      *)
        echo "0"
        ;;
    esac
    ;;
  *".pull_request.number // empty"*)
    echo "42"
    ;;
  *"contains("*)
    echo "123"
    ;;
  *"{body: .}"*)
    payload="$(cat)"
    printf '%s\n' "${payload}" > "${JQ_BODY_LOG}"
    echo '{"body":"mock"}'
    ;;
  *)
    cat
    ;;
esac
"#,
    );

    write_executable(
        &bin_dir.join("curl"),
        r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "${CURL_LOG}"
case "$*" in
  *"/issues/42/comments"*"-X PATCH"*)
    echo '{}'
    ;;
  *"/issues/comments/123"*)
    echo '{}'
    ;;
  *"/issues/42/comments"*)
    echo '[]'
    ;;
  *)
    echo '{}'
    ;;
esac
"#,
    );

    let report_file = temp
        .path()
        .join(".github-actionspec-dashboard/current/validation-report.json");
    let dashboard_file = temp
        .path()
        .join(".github-actionspec-dashboard/current/dashboard.md");
    let output_file = temp.path().join("github_output.txt");
    let summary_file = temp.path().join("step_summary.md");
    let event_file = temp.path().join("event.json");
    let curl_log = temp.path().join("curl.log");
    let jq_body_log = temp.path().join("jq-body.log");

    fs::write(&event_file, r#"{"pull_request":{"number":42}}"#).unwrap();
    fs::write(&curl_log, "").unwrap();
    fs::write(&jq_body_log, "").unwrap();

    let status = Command::new("/bin/sh")
        .arg("scripts/action/entrypoint.sh")
        .arg("validate-repo")
        .arg("--repo")
        .arg(".")
        .arg("--workflow")
        .arg("ci.yml")
        .arg("--actual")
        .arg("tests/fixtures/ci/ci-main-success.json")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env(
            "PATH",
            format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap()),
        )
        .env("INPUT_REPORT_FILE", &report_file)
        .env("INPUT_DASHBOARD_FILE", &dashboard_file)
        .env("INPUT_COMMENT_TITLE", "Workflow Matrix Dashboard")
        .env("INPUT_COMMENT_TAG", "test-matrix")
        .env("INPUT_GITHUB_TOKEN", "token")
        .env("GITHUB_OUTPUT", &output_file)
        .env("GITHUB_STEP_SUMMARY", &summary_file)
        .env("GITHUB_EVENT_PATH", &event_file)
        .env("GITHUB_REPOSITORY", "v4lproik/github-actionspec-rs")
        .env("CURL_LOG", &curl_log)
        .env("JQ_BODY_LOG", &jq_body_log)
        .status()
        .unwrap();

    assert!(status.success());
    assert!(report_file.exists());
    assert!(dashboard_file.exists());

    let outputs = fs::read_to_string(&output_file).unwrap();
    assert!(outputs.contains(&format!("report-path={}", report_file.display())));
    assert!(outputs.contains(&format!("dashboard-path={}", dashboard_file.display())));

    let summary = fs::read_to_string(&summary_file).unwrap();
    assert!(summary.contains("Validation Matrix"));

    let curl_log = fs::read_to_string(&curl_log).unwrap();
    assert!(curl_log.contains("/issues/42/comments"));
    assert!(curl_log.contains("/issues/comments/123"));

    let comment_body = fs::read_to_string(&jq_body_log).unwrap();
    assert!(comment_body.contains("Current: `1` payloads, `1` passed, `0` failed"));
    assert!(comment_body.contains("## Validation Matrix"));
}
