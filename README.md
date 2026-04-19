# github-actionspec-rs

[![CI](https://github.com/v4lproik/github-actionspec-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/v4lproik/github-actionspec-rs/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/v4lproik/github-actionspec-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/v4lproik/github-actionspec-rs)
[![Docker Hub](https://img.shields.io/docker/pulls/v4lproik/github-actionspec-rs)](https://hub.docker.com/r/v4lproik/github-actionspec-rs)

Validate GitHub Actions workflow behavior against CUE contracts.

`github-actionspec-rs` keeps expected workflow behavior in CUE, captures actual workflow payloads as JSON, and validates both to produce a report and a dashboard that engineers can review.

Project site: https://v4lproik.github.io/github-actionspec-rs/

## Why

- Catch workflow drift before it breaks automation.
- Review matrix, output, and cross-job behavior as artifacts instead of logs.
- Lint reusable workflow interfaces with `validate-callers`.
- Keep the contract layer explicit and easy to inspect.

Because the contract layer is expressed in CUE rather than ad hoc shell logic, the rules are usually easier for both humans and AI-assisted tooling to inspect, explain, and extend. This is a practical ergonomics point, not a benchmark claim.

## The Model

The product has one chain:

1. Declare expected behavior in `.cue`.
2. Capture actual workflow behavior as a normalized JSON payload.
3. Validate expected vs actual and render artifacts.

Minimal mental model:

```text
.github/workflows/ci.yml
  -> .github/actionspec/ci/main.cue
  -> tests/fixtures/ci/baseline.json
  -> target/actionspec/validation-report.json
  -> target/actionspec/dashboard.md
```

## Quickstart

Start with one workflow and one payload.

Bootstrap a starter contract and baseline payload:

```bash
just bootstrap-actionspec . ci.yml
```

That writes:

- `.github/actionspec/ci/main.cue`
- `tests/fixtures/ci/baseline.json`
- `.github/actionspec/ci/bootstrap-ci-snippet.yml`

Validate that payload and render both artifacts:

```bash
just validate-repo-dashboard . ci.yml \
  tests/fixtures/ci/baseline.json \
  target/actionspec/validation-report.json \
  target/actionspec/dashboard.md
```

If you want the smallest GitHub Action integration, start from the generated snippet or use:

```yaml
- uses: actions/checkout@v6

- name: Validate ci.yml contract
  id: actionspec
  uses: v4lproik/github-actionspec-rs@main
  with:
    workflow: ci.yml
    actual: tests/fixtures/ci/baseline.json
    report-file: /github/runner_temp/actionspec/current/validation-report.json
    dashboard-file: /github/runner_temp/actionspec/current/dashboard.md
```

## What You Get

- A JSON validation report with workflow metadata, job results, matrix labels, outputs, and typed issues.
- A markdown dashboard that fits job summaries, uploaded artifacts, and PR comments.
- Baseline diff support so a PR can show what changed from an earlier run.
- Runtime-backed validation once you want to validate a real CI run on `main`.

Failed payloads also carry a structured `issues` list in the report. Common CUE failures are classified as:

- `value_conflict`
- `unexpected_field`
- `missing_field`
- `constraint_violation`
- `cue_error`

The dashboard starts with a short issue summary so reviewers can see what kind of failures dominate a run before scanning individual payload rows.

## Choose The Right Mode

| Goal | Use |
| --- | --- |
| Lint reusable workflow callers | `validate-callers` |
| Validate one contract against one or more payloads | `validate-repo` |
| Produce one fragment per job in Actions | `emit-fragment` |
| Merge fragments into one normalized payload | `capture` |
| Render a PR-friendly matrix | `dashboard` |
| Start a contract test from an existing workflow | `bootstrap` |

`validate-repo` accepts individual files, directories, globs, and newline-separated file lists. That makes it easy to validate one payload locally or a whole fixture set in CI.

## Docs

- [Overview](https://v4lproik.github.io/github-actionspec-rs/)
- [Getting Started](https://v4lproik.github.io/github-actionspec-rs/getting-started.html)
- [Workflow Flow](https://v4lproik.github.io/github-actionspec-rs/workflow-flow.html)
- [Reports](https://v4lproik.github.io/github-actionspec-rs/reports.html)
- [Troubleshooting](https://v4lproik.github.io/github-actionspec-rs/troubleshooting.html)
- [CI Reference](https://v4lproik.github.io/github-actionspec-rs/ci-reference.html)

## Maintainers

Repository-specific development commands, CI rules, and PR conventions now live in [CONTRIBUTING.md](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/CONTRIBUTING.md).
