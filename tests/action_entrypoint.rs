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
  args_log=""
  prev=""
  for arg in "$@"; do
    if [ "$prev" = "--output" ]; then
      output="$arg"
    fi
    args_log="$args_log $arg"
    prev="$arg"
  done
  printf '%s\n' "$args_log" > "${DASHBOARD_ARGS_LOG}"
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
    let dashboard_args_log = temp.path().join("dashboard-args.log");

    fs::write(&event_file, r#"{"pull_request":{"number":42}}"#).unwrap();
    fs::write(&curl_log, "").unwrap();
    fs::write(&jq_body_log, "").unwrap();
    fs::write(&dashboard_args_log, "").unwrap();

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
        .env(
            "INPUT_DASHBOARD_OUTPUT_KEYS",
            "contract_build\nartifact_name",
        )
        .env("INPUT_COMMENT_PR", "true")
        .env("INPUT_COMMENT_TITLE", "Workflow Matrix Dashboard")
        .env("INPUT_COMMENT_TAG", "test-matrix")
        .env("INPUT_GITHUB_TOKEN", "token")
        .env("GITHUB_OUTPUT", &output_file)
        .env("GITHUB_STEP_SUMMARY", &summary_file)
        .env("GITHUB_EVENT_PATH", &event_file)
        .env("GITHUB_REPOSITORY", "v4lproik/github-actionspec-rs")
        .env("CURL_LOG", &curl_log)
        .env("JQ_BODY_LOG", &jq_body_log)
        .env("DASHBOARD_ARGS_LOG", &dashboard_args_log)
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

    let dashboard_args = fs::read_to_string(&dashboard_args_log).unwrap();
    assert!(dashboard_args.contains("--output-key contract_build"));
    assert!(dashboard_args.contains("--output-key artifact_name"));

    let curl_log = fs::read_to_string(&curl_log).unwrap();
    assert!(curl_log.contains("/issues/42/comments"));
    assert!(curl_log.contains("/issues/comments/123"));

    let comment_body = fs::read_to_string(&jq_body_log).unwrap();
    assert!(comment_body.contains("Current: `1` payloads, `1` passed, `0` failed"));
    assert!(comment_body.contains("## Validation Matrix"));
}

#[test]
fn action_entrypoint_captures_workflow_payload_and_writes_capture_output() {
    let temp = tempdir().unwrap();
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    write_executable(
        &bin_dir.join("github-actionspec"),
        r#"#!/bin/sh
set -eu
cmd="$1"
shift
if [ "$cmd" = "capture" ]; then
  output=""
  args_log=""
  prev=""
  for arg in "$@"; do
    if [ "$prev" = "--output" ]; then
      output="$arg"
    fi
    args_log="$args_log $arg"
    prev="$arg"
  done
  printf '%s\n' "$args_log" > "${CAPTURE_ARGS_LOG}"
  mkdir -p "$(dirname "$output")"
  cat > "$output" <<'EOF'
{
  "run": {
    "workflow": "ci.yml",
    "ref": "main",
    "inputs": {
      "run_ci": "true"
    },
    "jobs": {
      "build": {
        "result": "success"
      },
      "tests": {
        "result": "success"
      }
    }
  }
}
EOF
  exit 0
fi
if [ "$cmd" = "dashboard" ]; then
  echo "dashboard should not run in capture mode" >&2
  exit 7
fi
exit 1
"#,
    );

    let capture_file = temp
        .path()
        .join(".github-actionspec-capture/current/workflow-run.json");
    let output_file = temp.path().join("github_output.txt");
    let args_log = temp.path().join("capture-args.log");

    fs::write(&args_log, "").unwrap();

    let status = Command::new("/bin/sh")
        .arg("scripts/action/entrypoint.sh")
        .arg("capture")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env(
            "PATH",
            format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap()),
        )
        .env("INPUT_WORKFLOW", "ci.yml")
        .env("INPUT_REF_NAME", "main")
        .env("INPUT_CAPTURE_INPUTS", "run_ci=true")
        .env(
            "INPUT_CAPTURE_JOB_FILES",
            ".github/actionspec-fragments/build.json\n.github/actionspec-fragments/tests.json",
        )
        .env("INPUT_CAPTURE_FILE", &capture_file)
        .env("GITHUB_OUTPUT", &output_file)
        .env("CAPTURE_ARGS_LOG", &args_log)
        .status()
        .unwrap();

    assert!(status.success());
    assert!(capture_file.exists());

    let outputs = fs::read_to_string(&output_file).unwrap();
    assert!(outputs.contains(&format!("capture-path={}", capture_file.display())));
    assert!(!outputs.contains("report-path="));
    assert!(!outputs.contains("dashboard-path="));

    let args = fs::read_to_string(&args_log).unwrap();
    assert!(args.contains("--workflow ci.yml"));
    assert!(args.contains("--ref main"));
    assert!(args.contains("--input run_ci=true"));
    assert!(args.contains("--job-file .github/actionspec-fragments/build.json"));
    assert!(args.contains("--job-file .github/actionspec-fragments/tests.json"));
}
