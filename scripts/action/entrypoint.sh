#!/bin/sh
set -eu

lower_bool() {
  printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]'
}

write_outputs() {
  if [ -z "${GITHUB_OUTPUT:-}" ]; then
    return
  fi

  {
    printf 'report-path=%s\n' "$REPORT_FILE"
    printf 'dashboard-path=%s\n' "$DASHBOARD_FILE"
  } >> "${GITHUB_OUTPUT}"
}

write_summary() {
  if [ "$(lower_bool "${INPUT_WRITE_SUMMARY:-true}")" != "true" ] || [ -z "${GITHUB_STEP_SUMMARY:-}" ]; then
    return
  fi

  cat "${DASHBOARD_FILE}" >> "${GITHUB_STEP_SUMMARY}"
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

REPORT_FILE="${INPUT_REPORT_FILE:-.github-actionspec-dashboard/current/validation-report.json}"
DASHBOARD_FILE="${INPUT_DASHBOARD_FILE:-.github-actionspec-dashboard/current/dashboard.md}"
BASELINE_REPORT="${INPUT_BASELINE_REPORT:-}"

mkdir -p "$(dirname "${REPORT_FILE}")" "$(dirname "${DASHBOARD_FILE}")"

set +e
github-actionspec "$@" --report-file "${REPORT_FILE}"
status=$?
set -e

if [ -f "${REPORT_FILE}" ]; then
  if [ -n "${BASELINE_REPORT}" ] && [ -f "${BASELINE_REPORT}" ]; then
    github-actionspec dashboard \
      --current "${REPORT_FILE}" \
      --baseline "${BASELINE_REPORT}" \
      --output "${DASHBOARD_FILE}"
  else
    github-actionspec dashboard \
      --current "${REPORT_FILE}" \
      --output "${DASHBOARD_FILE}"
  fi

  write_outputs
  write_summary
  upsert_pr_comment
fi

exit "${status}"
