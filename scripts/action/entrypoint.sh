#!/bin/sh
set -eu

lower_bool() {
  printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]'
}

append_repeated_args() {
  value="${1:-}"
  flag="${2:-}"
  target_var="${3:-}"

  if [ -z "${value}" ] || [ -z "${flag}" ] || [ -z "${target_var}" ]; then
    return
  fi

  args=""
  old_ifs=$IFS
  IFS='
,'
  for item in ${value}; do
    trimmed="$(printf '%s' "$item" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
    [ -n "${trimmed}" ] || continue
    args="${args} ${flag} ${trimmed}"
  done
  IFS=$old_ifs

  eval "${target_var}=\"\${args}\""
}

write_outputs() {
  if [ -z "${GITHUB_OUTPUT:-}" ]; then
    return
  fi

  {
    if [ -n "${CAPTURE_FILE:-}" ]; then
      printf 'capture-path=%s\n' "$CAPTURE_FILE"
    fi
    if [ -n "${REPORT_FILE:-}" ]; then
      printf 'report-path=%s\n' "$REPORT_FILE"
    fi
    if [ -n "${DASHBOARD_FILE:-}" ]; then
      printf 'dashboard-path=%s\n' "$DASHBOARD_FILE"
    fi
  } >> "${GITHUB_OUTPUT}"
}

write_summary() {
  if [ "$(lower_bool "${INPUT_WRITE_SUMMARY:-true}")" != "true" ] || [ -z "${GITHUB_STEP_SUMMARY:-}" ]; then
    return
  fi

  cat "${DASHBOARD_FILE}" >> "${GITHUB_STEP_SUMMARY}"
}

report_stat() {
  report_path="$1"
  status_name="$2"
  jq -r --arg status_name "${status_name}" '[.actuals[] | select(.status == $status_name)] | length' "${report_path}"
}

render_pr_summary() {
  current_total="$(jq -r '.actuals | length' "${REPORT_FILE}")"
  current_passed="$(report_stat "${REPORT_FILE}" "passed")"
  current_failed="$(report_stat "${REPORT_FILE}" "failed")"
  workflow_name="$(jq -r '.workflow' "${REPORT_FILE}")"
  declaration_path="$(jq -r '.declaration_path' "${REPORT_FILE}")"

  printf -- '- Workflow: `%s`\n' "${workflow_name}"
  printf -- '- Declaration: `%s`\n' "${declaration_path}"
  printf -- '- Current: `%s` payloads, `%s` passed, `%s` failed\n' \
    "${current_total}" "${current_passed}" "${current_failed}"

  if [ -n "${BASELINE_REPORT}" ] && [ -f "${BASELINE_REPORT}" ]; then
    baseline_total="$(jq -r '.actuals | length' "${BASELINE_REPORT}")"
    baseline_passed="$(report_stat "${BASELINE_REPORT}" "passed")"
    baseline_failed="$(report_stat "${BASELINE_REPORT}" "failed")"

    printf -- '- Baseline: `%s` payloads, `%s` passed, `%s` failed\n' \
      "${baseline_total}" "${baseline_passed}" "${baseline_failed}"
    printf -- '- Delta: passed `%+d`, failed `%+d`\n' \
      "$((current_passed - baseline_passed))" \
      "$((current_failed - baseline_failed))"
  fi
}

upsert_pr_comment() {
  if [ "$(lower_bool "${INPUT_COMMENT_PR:-false}")" != "true" ]; then
    return
  fi

  if [ -z "${INPUT_GITHUB_TOKEN:-}" ] || [ -z "${GITHUB_REPOSITORY:-}" ] || [ -z "${GITHUB_EVENT_PATH:-}" ]; then
    return
  fi

  pr_number="$(jq -r '.pull_request.number // empty' "${GITHUB_EVENT_PATH}")"
  if [ -z "${pr_number}" ]; then
    return
  fi

  marker="<!-- github-actionspec-matrix:${INPUT_COMMENT_TAG:-github-actionspec-matrix} -->"
  title="${INPUT_COMMENT_TITLE:-Workflow Matrix Dashboard}"
  payload="$(
    {
      printf '%s\n' "${marker}"
      printf '## %s\n\n' "${title}"
      render_pr_summary
      printf '\n'
      cat "${DASHBOARD_FILE}"
    } | jq -Rs '{body: .}'
  )"

  comments_json="$(
    curl -fsSL \
      -H "Authorization: Bearer ${INPUT_GITHUB_TOKEN}" \
      -H "Accept: application/vnd.github+json" \
      "https://api.github.com/repos/${GITHUB_REPOSITORY}/issues/${pr_number}/comments"
  )"
  comment_id="$(
    printf '%s' "${comments_json}" |
      jq -r --arg marker "${marker}" '[.[] | select(.body | contains($marker))][0].id // empty'
  )"

  if [ -n "${comment_id}" ]; then
    curl -fsSL \
      -X PATCH \
      -H "Authorization: Bearer ${INPUT_GITHUB_TOKEN}" \
      -H "Accept: application/vnd.github+json" \
      -H "Content-Type: application/json" \
      "https://api.github.com/repos/${GITHUB_REPOSITORY}/issues/comments/${comment_id}" \
      -d "${payload}" >/dev/null
  else
    curl -fsSL \
      -X POST \
      -H "Authorization: Bearer ${INPUT_GITHUB_TOKEN}" \
      -H "Accept: application/vnd.github+json" \
      -H "Content-Type: application/json" \
      "https://api.github.com/repos/${GITHUB_REPOSITORY}/issues/${pr_number}/comments" \
      -d "${payload}" >/dev/null
  fi
}

MODE="${1:-${INPUT_MODE:-validate-repo}}"
CAPTURE_FILE=""
REPORT_FILE=""
DASHBOARD_FILE=""
BASELINE_REPORT="${INPUT_BASELINE_REPORT:-}"

case "${MODE}" in
  capture)
    CAPTURE_FILE="${INPUT_CAPTURE_FILE:-/github/runner_temp/github-actionspec-capture/current/workflow-run.json}"
    mkdir -p "$(dirname "${CAPTURE_FILE}")"
    CAPTURE_INPUT_ARGS=""
    CAPTURE_JOB_FILE_ARGS=""
    append_repeated_args "${INPUT_CAPTURE_INPUTS:-}" "--input" "CAPTURE_INPUT_ARGS"
    append_repeated_args "${INPUT_CAPTURE_JOB_FILES:-}" "--job-file" "CAPTURE_JOB_FILE_ARGS"

    set -- capture
    [ -n "${INPUT_WORKFLOW:-}" ] && set -- "$@" --workflow "${INPUT_WORKFLOW}"
    [ -n "${INPUT_REF_NAME:-}" ] && set -- "$@" --ref "${INPUT_REF_NAME}"
    if [ -n "${CAPTURE_INPUT_ARGS:-}" ]; then
      # shellcheck disable=SC2086
      set -- "$@" ${CAPTURE_INPUT_ARGS}
    fi
    if [ -n "${CAPTURE_JOB_FILE_ARGS:-}" ]; then
      # shellcheck disable=SC2086
      set -- "$@" ${CAPTURE_JOB_FILE_ARGS}
    fi
    set -- "$@" --output "${CAPTURE_FILE}"
    github-actionspec "$@"
    write_outputs
    ;;
  validate-repo)
    REPORT_FILE="${INPUT_REPORT_FILE:-/github/runner_temp/github-actionspec-dashboard/current/validation-report.json}"
    DASHBOARD_FILE="${INPUT_DASHBOARD_FILE:-/github/runner_temp/github-actionspec-dashboard/current/dashboard.md}"

    mkdir -p "$(dirname "${REPORT_FILE}")" "$(dirname "${DASHBOARD_FILE}")"

    set --
    [ -n "${INPUT_REPO:-}" ] && set -- "$@" --repo "${INPUT_REPO}"
    [ -n "${INPUT_WORKFLOW:-}" ] && set -- "$@" --workflow "${INPUT_WORKFLOW}"
    [ -n "${INPUT_ACTUAL:-}" ] && set -- "$@" --actual "${INPUT_ACTUAL}"
    [ -n "${INPUT_DECLARATIONS_DIR:-}" ] && set -- "$@" --declarations-dir "${INPUT_DECLARATIONS_DIR}"

    set +e
    github-actionspec validate-repo "$@" --report-file "${REPORT_FILE}"
    status=$?
    set -e

    if [ -f "${REPORT_FILE}" ]; then
      set --
      if [ -n "${BASELINE_REPORT}" ] && [ -f "${BASELINE_REPORT}" ]; then
        set -- "$@" --baseline "${BASELINE_REPORT}"
      fi
      DASHBOARD_ARGS=""
      append_repeated_args "${INPUT_DASHBOARD_OUTPUT_KEYS:-}" "--output-key" "DASHBOARD_ARGS"
      if [ -n "${DASHBOARD_ARGS:-}" ]; then
        # shellcheck disable=SC2086
        set -- "$@" ${DASHBOARD_ARGS}
      fi
      github-actionspec dashboard \
        --current "${REPORT_FILE}" \
        "$@" \
        --output "${DASHBOARD_FILE}"

      write_outputs
      write_summary
      upsert_pr_comment
    fi

    exit "${status}"
    ;;
  *)
    printf 'unsupported action mode: %s\n' "${MODE}" >&2
    exit 1
    ;;
esac
