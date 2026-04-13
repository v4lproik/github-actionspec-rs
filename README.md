# github-actionspec-rs

[![CI](https://github.com/v4lproik/github-actionspec-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/v4lproik/github-actionspec-rs/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/v4lproik/github-actionspec-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/v4lproik/github-actionspec-rs)
[![Docker Hub](https://img.shields.io/docker/pulls/v4lproik/github-actionspec-rs)](https://hub.docker.com/r/v4lproik/github-actionspec-rs)

Rust implementation of the GitHub Actions workflow contract validator.

It keeps CUE as the intermediate language and uses the `cue` CLI for validation.

Because the contract layer is expressed in CUE rather than ad hoc shell logic, the validation rules are usually easier for both humans and AI-assisted tooling to inspect, explain, and extend. This should be read as a practical ergonomics benefit, not as a benchmark claim that CUE is universally "better for AI".

Project site: https://v4lproik.github.io/github-actionspec-rs/

## Tooling

This repo uses Docker for build, lint, test, and coverage so local development and CI run the same toolchain. `just` remains the repository entrypoint. `mise` is only kept for host-side commands such as `discover` and `validate`.

```bash
just docker-build
just lint
just test
just ci
just discover
just coverage
just coverage-summary
```

The Docker image definition lives in [Dockerfile](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/Dockerfile), and the repository build target is declared in [docker-bake.hcl](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/docker-bake.hcl). The repo-local host configuration lives in [.mise.toml](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/.mise.toml) and is only needed for host-executed commands.

This repo also exposes the common commands through [justfile](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/justfile):

```bash
just install
just docker-build
just docker-build-runtime
just fmt
just lint
just ci
just test
just discover
just coverage-summary
just pr-create
just validate-repo /path/to/repo ci.yml /path/to/actual.json
just validate-repo-report /path/to/repo ci.yml /path/to/payloads target/actionspec/report.json
just dashboard-report target/actionspec/report.json target/actionspec/dashboard.md
```

Commands:

- `github-actionspec discover --repo <path>`
- `github-actionspec validate --schema <file> --schema <file> --contract <file> --actual <file-or-glob>`
- `github-actionspec validate-repo --repo <path> [--workflow <name>] --actual <file-dir-or-glob> [--report-file <report.json>]`
- `github-actionspec dashboard --current <report.json> [--baseline <report.json>] --output <dashboard.md>`

## GitHub Action

This repository also exposes a Docker-based GitHub Action for the common `validate-repo` flow. The action runs the bundled `github-actionspec` binary together with the bundled `cue` runtime, so the calling workflow only needs a checked out repository and a normalized JSON payload.

```yaml
- uses: actions/checkout@v6

- name: Validate workflow contracts
  uses: v4lproik/github-actionspec-rs@main

- name: Validate one workflow explicitly
  uses: v4lproik/github-actionspec-rs@main
  with:
    workflow: ci.yml
    actual: .github/actionspec-artifacts/ci-main.json
```

By convention, the action defaults to:

- `repo: .`
- `declarations-dir: .github/actionspec`
- `actual: .github/actionspec-artifacts`
- inferring `workflow` from the payloads when they all belong to the same workflow

That means the shortest setup is just:

```yaml
- uses: actions/checkout@v6

- uses: v4lproik/github-actionspec-rs@main
```

Inputs:

- `repo`: target repository root containing `.github/actionspec` declarations. Defaults to `.`
- `workflow`: workflow file name to validate. Optional when the provided payloads all belong to the same workflow
- `actual`: path to one normalized workflow run JSON payload, a directory containing JSON payloads, a glob pattern, or a newline-separated list of payloads and glob patterns. Defaults to `.github/actionspec-artifacts`
- `declarations-dir`: custom declarations directory. Defaults to `.github/actionspec`
- `report-file`: path where the action writes the JSON validation report. Defaults to `/github/runner_temp/github-actionspec-dashboard/current/validation-report.json`
- `baseline-report`: optional path to a previous JSON validation report used to compute matrix diffs
- `dashboard-file`: path where the action writes the markdown matrix dashboard. Defaults to `/github/runner_temp/github-actionspec-dashboard/current/dashboard.md`
- `write-summary`: whether to append the matrix dashboard to the job summary. Defaults to `true`
- `comment-pr`: whether to upsert the matrix dashboard as a PR comment. Defaults to `false`
- `comment-title`: title used for the PR comment. Defaults to `Workflow Matrix Dashboard`
- `comment-tag`: stable marker used to find and update the existing PR comment. Defaults to `github-actionspec-matrix`
- `github-token`: token used for PR comment upserts when `comment-pr` is enabled

Examples:

```yaml
- name: Validate one payload
  uses: v4lproik/github-actionspec-rs@main
  with:
    workflow: ci.yml
    actual: .github/actionspec-artifacts/ci-main.json

- name: Validate a whole folder of payloads
  uses: v4lproik/github-actionspec-rs@main
  with:
    workflow: ci.yml
    actual: .github/actionspec-artifacts/passing

- name: Validate using the default artifacts directory and inferred workflow
  uses: v4lproik/github-actionspec-rs@main

- name: Validate an explicit list of payloads
  uses: v4lproik/github-actionspec-rs@main
  with:
    actual: |
      .github/actionspec-artifacts/ci-main.json
      .github/actionspec-artifacts/ci-main-pages.json

- name: Validate payloads through a glob pattern
  uses: v4lproik/github-actionspec-rs@main
  with:
    repo: .
    workflow: ci.yml
    actual: .github/actionspec-artifacts/**/*.json

- name: Validate, diff against a previous report, and comment on the PR
  id: actionspec
  uses: v4lproik/github-actionspec-rs@main
  with:
    repo: .
    workflow: ci.yml
    actual: .github/actionspec-artifacts/**/*.json
    report-file: ${{ runner.temp }}/github-actionspec-dashboard/current/validation-report.json
    baseline-report: ${{ runner.temp }}/github-actionspec-dashboard/baseline/validation-report.json
    dashboard-file: ${{ runner.temp }}/github-actionspec-dashboard/current/dashboard.md
    comment-pr: true
    github-token: ${{ github.token }}

- name: Upload the matrix artifact
  uses: actions/upload-artifact@v4
  with:
    name: ci-matrix-dashboard
    path: |
      ${{ steps.actionspec.outputs.report-path }}
      ${{ steps.actionspec.outputs.dashboard-path }}
```

To show the difference between the current and previous matrix, download the earlier report artifact before the action step and pass its report JSON through `baseline-report`. The action updates a single PR comment identified by `comment-tag`, so the discussion stays in one place instead of growing a new comment on every push.

Example:

```yaml
- name: Download previous matrix artifact
  uses: dawidd6/action-download-artifact@v9
  with:
    workflow: ci.yml
    branch: main
    name: ci-matrix-dashboard
    path: ${{ runner.temp }}/github-actionspec-dashboard/baseline
    if_no_artifact_found: warn
```

## Coverage

The target for this repo is to stay close to `90%` test coverage.

Use:

```bash
just coverage
just coverage-summary
```

These commands run inside the repository Docker image. Use `just coverage` for the HTML report and `just coverage-summary` for the terminal summary.

For CI and Codecov uploads, use:

```bash
just coverage-ci
```

This emits `target/llvm-cov/lcov.info`, which the repository workflow uploads to Codecov.

## CI

GitHub Actions must call `just`, not raw `cargo`, `gh`, or `mise` command sequences.

- The workflow starts with a `detect-changes` job powered by `dorny/paths-filter` and filter rules stored in `.github/filters/changes.yml`.
- Build, lint, test, remote action integration, runtime verification, docker publish, and Pages run in that order when the relevant change filters match.
- The workflow can also be started manually through `workflow_dispatch`; manual runs force the full CI path even if no matching file changes are present.
- Build: `just build`
- Lint: `just lint`
- Test: `just test`
- Matrix report: `just validate-repo-report . ci.yml tests/fixtures/ci target/actionspec/validation-report.json`
- Matrix dashboard: `just dashboard-report target/actionspec/validation-report.json target/actionspec/dashboard.md`
- Coverage upload: `just coverage-ci`
- Local full pass: `just ci`
- The remote action integration check now lives inside the main `CI` workflow and validates the published `v4lproik/github-actionspec-rs@main` action reference end to end against this repository's own `ci.yml` fixtures on pushes to `main`.
- The `tests` job publishes a `ci-matrix-dashboard` artifact with the current validation report and a markdown matrix. On pull requests, the workflow also updates a single PR comment with that matrix and diffs it against the latest available baseline artifact.

## Docker Parity

The Docker-backed commands mount the repository into `/workspace` and preserve host file ownership by running the container with the current user id. Cargo cache data is stored under `.docker-cache/`, which is gitignored. Image builds are routed through `docker buildx bake` so the build definition stays centralized in the bake file.

## Runtime Image

The repository also exposes a runtime image target for the CLI itself:

```bash
just docker-build-runtime
just docker-run-runtime
```

The runtime target includes both `github-actionspec` and the `cue` CLI, so commands such as `validate` and `validate-repo` work inside the image without requiring extra host tooling.

If you want to publish a public image to Docker Hub, the repository already exposes a push entrypoint:

```bash
just docker-push-runtime docker.io/<namespace>/github-actionspec-rs:latest
```

The CI workflow now verifies the runtime image through `just runtime-ci`. Publication is gated on successful checks and coverage for pushes to `main`, and the publish job is skipped entirely if the workflow is cancelled or if Docker Hub credentials are not configured.

Docker documents Docker Hub public repositories as unlimited on the free tier, subject to fair use. Source: [Docker Hub docs](https://docs.docker.com/docker-hub/) and [Docker pricing](https://www.docker.com/pricing/).

## Pull Requests

Open repository PRs through `just` so the command surface stays centralized:

```bash
just pr-create
just pr-create-draft
```

Both recipes default the base branch to `main`.
