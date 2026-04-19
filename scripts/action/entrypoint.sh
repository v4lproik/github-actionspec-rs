#!/bin/sh
set -eu

lower_bool() {
  printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]'
}

action_temp_root() {
  for candidate in "${RUNNER_TEMP:-}" "/github/runner_temp" "${TMPDIR:-}" "/tmp"; do
    [ -n "${candidate}" ] || continue
    if [ -d "${candidate}" ] && [ -w "${candidate}" ]; then
      printf '%s' "${candidate%/}"
      return
    fi
  done

  printf '%s' "/tmp"
}

default_temp_path() {
  root="$(action_temp_root)"
  suffix="${1:-}"

  if [ -z "${suffix}" ]; then
    printf '%s' "${root}"
    return
  fi

  printf '%s/%s' "${root}" "${suffix}"
}

input_value() {
  input_name="${1:-}"
  default_value="${2-}"

  if [ -z "${input_name}" ]; then
    printf '%s' "${default_value}"
    return
  fi

  raw_name="INPUT_${input_name}"
  legacy_name="$(printf 'INPUT_%s' "${input_name}" | tr '-' '_')"

  # GitHub action inputs are exposed as INPUT_<NAME>, but shell runtimes vary in
  # how they preserve non-POSIX names containing hyphens. Check the raw key
  # first, then fall back to the underscored variant used by portable shells.
  raw_value="$(printenv "${raw_name}" 2>/dev/null || true)"
  if [ -n "${raw_value}" ]; then
    printf '%s' "${raw_value}"
    return
  fi

  eval "legacy_value=\${${legacy_name}:-}"
  if [ -n "${legacy_value}" ]; then
    printf '%s' "${legacy_value}"
    return
  fi

  printf '%s' "${default_value}"
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

resolve_dashboard_file() {
  configured_dashboard_file="$(input_value 'DASHBOARD-FILE')"
  if [ -n "${configured_dashboard_file}" ]; then
    printf '%s' "${configured_dashboard_file}"
    return
  fi

  report_dir="$(dirname "${REPORT_FILE}")"
  printf '%s/dashboard.md' "${report_dir}"
}

write_outputs() {
  if [ -z "${GITHUB_OUTPUT:-}" ]; then
    return
  fi

  {
    if [ -n "${FRAGMENT_FILE:-}" ]; then
      printf 'fragment-path=%s\n' "$FRAGMENT_FILE"
    fi
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
  if [ "$(lower_bool "$(input_value 'WRITE-SUMMARY' 'true')")" != "true" ] || [ -z "${GITHUB_STEP_SUMMARY:-}" ]; then
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
  if [ "$(lower_bool "$(input_value 'COMMENT-PR' 'false')")" != "true" ]; then
    return
  fi

  github_token="$(input_value 'GITHUB-TOKEN')"
  if [ -z "${github_token}" ] || [ -z "${GITHUB_REPOSITORY:-}" ] || [ -z "${GITHUB_EVENT_PATH:-}" ]; then
    return
  fi

  pr_number="$(jq -r '.pull_request.number // empty' "${GITHUB_EVENT_PATH}")"
  if [ -z "${pr_number}" ]; then
    return
  fi

  marker="<!-- github-actionspec-matrix:$(input_value 'COMMENT-TAG' 'github-actionspec-matrix') -->"
  title="$(input_value 'COMMENT-TITLE' 'Workflow Matrix Dashboard')"
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
      -H "Authorization: Bearer ${github_token}" \
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
      -H "Authorization: Bearer ${github_token}" \
      -H "Accept: application/vnd.github+json" \
      -H "Content-Type: application/json" \
      "https://api.github.com/repos/${GITHUB_REPOSITORY}/issues/comments/${comment_id}" \
      -d "${payload}" >/dev/null
  else
    curl -fsSL \
      -X POST \
      -H "Authorization: Bearer ${github_token}" \
      -H "Accept: application/vnd.github+json" \
      -H "Content-Type: application/json" \
      "https://api.github.com/repos/${GITHUB_REPOSITORY}/issues/${pr_number}/comments" \
      -d "${payload}" >/dev/null
  fi
}

MODE="${1:-$(input_value 'MODE' 'validate-repo')}"
FRAGMENT_FILE=""
CAPTURE_FILE=""
REPORT_FILE=""
DASHBOARD_FILE=""
BASELINE_REPORT="$(input_value 'BASELINE-REPORT')"

case "${MODE}" in
  emit-fragment)
    FRAGMENT_FILE="$(input_value 'EMIT-FILE' "$(default_temp_path 'github-actionspec-fragments/current/job.json')")"
    mkdir -p "$(dirname "${FRAGMENT_FILE}")"
    EMIT_OUTPUT_ARGS=""
    EMIT_MATRIX_ARGS=""
    EMIT_STEP_CONCLUSION_ARGS=""
    EMIT_STEP_OUTPUT_ARGS=""
    append_repeated_args "$(input_value 'EMIT-OUTPUTS')" "--output" "EMIT_OUTPUT_ARGS"
    append_repeated_args "$(input_value 'EMIT-MATRIX')" "--matrix" "EMIT_MATRIX_ARGS"
    append_repeated_args "$(input_value 'EMIT-STEP-CONCLUSIONS')" "--step-conclusion" "EMIT_STEP_CONCLUSION_ARGS"
    append_repeated_args "$(input_value 'EMIT-STEP-OUTPUTS')" "--step-output" "EMIT_STEP_OUTPUT_ARGS"

    set -- emit-fragment
    emit_job="$(input_value 'EMIT-JOB')"
    emit_result="$(input_value 'EMIT-RESULT')"
    [ -n "${emit_job}" ] && set -- "$@" --job "${emit_job}"
    [ -n "${emit_result}" ] && set -- "$@" --result "${emit_result}"
    if [ -n "${EMIT_OUTPUT_ARGS:-}" ]; then
      # shellcheck disable=SC2086
      set -- "$@" ${EMIT_OUTPUT_ARGS}
    fi
    if [ -n "${EMIT_MATRIX_ARGS:-}" ]; then
      # shellcheck disable=SC2086
      set -- "$@" ${EMIT_MATRIX_ARGS}
    fi
    if [ -n "${EMIT_STEP_CONCLUSION_ARGS:-}" ]; then
      # shellcheck disable=SC2086
      set -- "$@" ${EMIT_STEP_CONCLUSION_ARGS}
    fi
    if [ -n "${EMIT_STEP_OUTPUT_ARGS:-}" ]; then
      # shellcheck disable=SC2086
      set -- "$@" ${EMIT_STEP_OUTPUT_ARGS}
    fi
    set -- "$@" --file "${FRAGMENT_FILE}"
    github-actionspec "$@"
    write_outputs
    ;;
  capture)
    CAPTURE_FILE="$(input_value 'CAPTURE-FILE' "$(default_temp_path 'github-actionspec-capture/current/workflow-run.json')")"
    mkdir -p "$(dirname "${CAPTURE_FILE}")"
    CAPTURE_INPUT_ARGS=""
    CAPTURE_JOB_FILE_ARGS=""
    append_repeated_args "$(input_value 'CAPTURE-INPUTS')" "--input" "CAPTURE_INPUT_ARGS"
    append_repeated_args "$(input_value 'CAPTURE-JOB-FILES')" "--job-file" "CAPTURE_JOB_FILE_ARGS"

    set -- capture
    workflow_input="$(input_value 'WORKFLOW')"
    ref_name_input="$(input_value 'REF-NAME')"
    [ -n "${workflow_input}" ] && set -- "$@" --workflow "${workflow_input}"
    [ -n "${ref_name_input}" ] && set -- "$@" --ref "${ref_name_input}"
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
    REPORT_FILE="$(input_value 'REPORT-FILE' "$(default_temp_path 'github-actionspec-dashboard/current/validation-report.json')")"
    DASHBOARD_FILE="$(resolve_dashboard_file)"

    mkdir -p "$(dirname "${REPORT_FILE}")" "$(dirname "${DASHBOARD_FILE}")"

    set --
    repo_input="$(input_value 'REPO')"
    workflow_input="$(input_value 'WORKFLOW')"
    actual_input="$(input_value 'ACTUAL')"
    declarations_dir_input="$(input_value 'DECLARATIONS-DIR')"
    [ -n "${repo_input}" ] && set -- "$@" --repo "${repo_input}"
    [ -n "${workflow_input}" ] && set -- "$@" --workflow "${workflow_input}"
    [ -n "${actual_input}" ] && set -- "$@" --actual "${actual_input}"
    [ -n "${declarations_dir_input}" ] && set -- "$@" --declarations-dir "${declarations_dir_input}"

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
      append_repeated_args "$(input_value 'DASHBOARD-OUTPUT-KEYS')" "--output-key" "DASHBOARD_ARGS"
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
