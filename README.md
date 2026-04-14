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
just emit-fragment build success .github/actionspec-fragments/build.json
just capture ci.yml .github/actionspec-artifacts/ci-main.json .github/actionspec-fragments
just coverage
just coverage-summary
```

The Docker image definition lives in [Dockerfile](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/Dockerfile), and the repository build target is declared in [docker-bake.hcl](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/docker-bake.hcl). The repo-local host configuration lives in [.mise.toml](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/.mise.toml) and is only needed for host-executed commands.
The dev and runtime images both install the `cue` version pinned in `.mise.toml`, so `just test` covers real CUE evaluation in addition to the shim-based validation tests.

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
just emit-fragment build success .github/actionspec-fragments/build.json
just capture ci.yml .github/actionspec-artifacts/ci-main.json .github/actionspec-fragments
just coverage-summary
just pr-create
just validate-callers /path/to/repo
just validate-callers-report /path/to/repo target/actionspec/callers-report.json
just validate-repo /path/to/repo ci.yml /path/to/actual.json
just validate-repo-report /path/to/repo ci.yml /path/to/payloads target/actionspec/report.json
just validate-repo-report-dry /path/to/repo ci.yml /path/to/payloads target/actionspec/report.json
just dashboard-report target/actionspec/report.json target/actionspec/dashboard.md
```

Commands:

- `github-actionspec emit-fragment --job <name> --result <status> --file <fragment.json>`
- `github-actionspec capture --workflow <name> --job-file <file-dir-or-glob> --output <actual.json>`
- `github-actionspec discover --repo <path>`
- `github-actionspec validate-callers --repo <path> [--report-file <report.json>] [--dry-run]`
- `github-actionspec validate --schema <file> --schema <file> --contract <file> --actual <file-or-glob>`
- `github-actionspec validate-repo --repo <path> [--workflow <name>] --actual <file-dir-or-glob> [--report-file <report.json>] [--dry-run]`
- `github-actionspec dashboard --current <report.json> [--baseline <report.json>] [--output-key <name>] --output <dashboard.md>`

## Emit Fragments

The intended producer flow is one job, one fragment, one final capture step.

`emit-fragment` writes the per-job JSON fragment that `capture` already understands, so calling repositories do not need to hand-roll the shape themselves.

Basic fragment:

```bash
just emit-fragment build success .github/actionspec-fragments/build.json
```

Fragment with outputs, matrix values, and step outputs:

```bash
github-actionspec emit-fragment \
  --job build \
  --result success \
  --output contract_build=build-ts-service \
  --output artifact_name=build-ts-service-linux-amd64 \
  --matrix app=build-ts-service \
  --matrix shard=2 \
  --step-conclusion compile=success \
  --step-output compile.digest=sha256:abc123 \
  --file .github/actionspec-fragments/build.json
```

This writes:

```json
{
  "job": "build",
  "result": "success",
  "outputs": {
    "artifact_name": "build-ts-service-linux-amd64",
    "contract_build": "build-ts-service"
  },
  "matrix": {
    "app": "build-ts-service",
    "shard": 2
  },
  "steps": {
    "compile": {
      "conclusion": "success",
      "outputs": {
        "digest": "sha256:abc123"
      }
    }
  }
}
```

`--matrix` accepts JSON scalars and arrays when possible, then falls back to strings. This means `shard=2` is written as a number, `enabled=true` as a boolean, and `app=build-ts-service` as a string.

## Capture Payloads

The easiest way to make workflow validation broadly usable is to standardize the payload generation step.

`capture` merges one JSON fragment per job into the normalized `run` payload that `validate-repo` already understands. A fragment looks like:

```json
{
  "job": "build",
  "result": "success",
  "matrix": {
    "app": "build-ts-service",
    "target": "linux-amd64"
  },
  "outputs": {
    "contract_build": "build-ts-service"
  },
  "steps": {
    "compile": {
      "conclusion": "success",
      "outputs": {
        "digest": "sha256:abc123"
      }
    }
  }
}
```

Capture a full workflow payload from a directory of fragments with:

```bash
just capture ci.yml .github/actionspec-artifacts/ci-main.json .github/actionspec-fragments
```

The CLI also supports repeated workflow inputs and explicit refs:

```bash
github-actionspec capture \
  --workflow ci.yml \
  --ref main \
  --input run_ci=true \
  --input run_pages=false \
  --job-file .github/actionspec-fragments \
  --output .github/actionspec-artifacts/ci-main.json
```

Once captured, validate the resulting payload with the existing repo flow:

```bash
just validate-repo . ci.yml .github/actionspec-artifacts/ci-main.json
```

In GitHub Actions, the intended pattern is:

- each job writes one fragment JSON file
- the final aggregation job downloads those fragment artifacts
- the aggregation job runs `capture`
- the same job runs `validate-repo` and uploads the report or dashboard artifact

Reusable workflows can also be treated as static contracts. `validate-callers` scans local `uses: ./.github/workflows/*.yml` jobs and checks that callers still match the callee workflow's `workflow_call` inputs and outputs.

Example reusable workflow interface:

```yaml
on:
  workflow_call:
    inputs:
      environment:
        type: string
        required: true
    outputs:
      image_tag:
        value: ${{ jobs.build.outputs.image_tag }}
```

Example caller:

```yaml
jobs:
  build:
    uses: ./.github/workflows/reusable-build.yml
    with:
      environment: staging

  summarize:
    needs: [build]
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ needs.build.outputs.image_tag }}"
```

Validate those caller contracts locally with:

```bash
just validate-callers .
```

To keep the run non-blocking and inspect the full caller/callee analysis later, write a report in dry mode:

```bash
just validate-callers-report . target/actionspec/callers-report.json
```

The command reports:

- missing required reusable-workflow inputs
- unexpected caller inputs
- obvious literal type mismatches for `string`, `boolean`, and `number` inputs
- `needs.<job>.outputs.<name>` references to outputs the called workflow no longer exports

The caller report also preserves the static analysis surface for each reusable workflow job:

- caller workflow path
- called reusable workflow path
- provided `with:` inputs
- referenced `needs.<job>.outputs.*` values
- issues attached to that call

Matrix-aware contracts can assert both matrix dimensions and job outputs. For example, this contract keeps a `build-ts-service` matrix entry aligned with the emitted `contract_build` output:

```cue
package actionspec

run: #WorkflowRun & {
  workflow: "build.yml"
  jobs: {
    build: {
      result: "success"
      matrix: {
        app: "build-ts-service"
      }
      outputs: {
        contract_build: "build-ts-service"
      }
    }
  }
}
```

Matching normalized payload:

```json
{
  "run": {
    "workflow": "build.yml",
    "jobs": {
      "build": {
        "result": "success",
        "matrix": {
          "app": "build-ts-service"
        },
        "outputs": {
          "contract_build": "build-ts-service"
        }
      }
    }
  }
}
```

If the workflow emits a different output for that same matrix entry, validation fails. For example, this payload should be rejected because the matrix variant and the emitted contract name diverge:

```json
{
  "run": {
    "workflow": "build.yml",
    "jobs": {
      "build": {
        "result": "success",
        "matrix": {
          "app": "build-ts-service"
        },
        "outputs": {
          "contract_build": "contract-build"
        }
      }
    }
  }
}
```

Cross-job invariants work the same way. This contract says the `publish` job must reuse the exact image tag emitted by `build`:

```cue
package actionspec

run: #WorkflowRun & {
  workflow: "release.yml"
  jobs: {
    build: {
      result: "success"
      outputs: {
        image_tag: string
      }
    }
    publish: {
      result: "success"
      outputs: {
        published_tag: run.jobs.build.outputs.image_tag
      }
    }
  }
}
```

That pattern is useful when one workflow job promotes an artifact, image tag, or contract name produced by an earlier job and you want the contract to reject drift between them.

You can validate that pattern locally with:

```bash
github-actionspec validate \
  --schema schema/workflow_run.cue \
  --contract .github/actionspec/build/main.cue \
  --actual .github/actionspec-artifacts/build-ts-service.json
```

When you generate a validation report or dashboard, the matrix labels and job outputs are preserved and rendered so the PR comment can show which variant changed and what it emitted, for example `app=build-ts-service, target=linux-amd64` together with `build.contract_build=build-ts-service`.

To analyze workflow execution output without failing the command, run `validate-repo` in dry mode and keep the report:

```bash
just validate-repo-report-dry . ci.yml .github/actionspec-artifacts target/actionspec/validation-report.json
```

That still runs the full validation logic and records failures in the report, but exits successfully so you can inspect the produced values locally or upload the artifact from CI.

To keep the dashboard compact, you can choose which outputs appear:

```bash
github-actionspec dashboard \
  --current target/actionspec/report.json \
  --output-key contract_build \
  --output-key artifact_name \
  --output target/actionspec/dashboard.md
```

## GitHub Action

This repository also exposes a Docker-based GitHub Action for both `capture` and `validate-repo`. The action runs the bundled `github-actionspec` binary together with the bundled `cue` runtime, so the calling workflow only needs a checked out repository plus either job fragments or a normalized JSON payload.

```yaml
- uses: actions/checkout@v6

- name: Validate workflow contracts
  uses: v4lproik/github-actionspec-rs@main

- name: Capture a normalized payload from job fragments
  uses: v4lproik/github-actionspec-rs@main
  with:
    mode: capture
    workflow: ci.yml
    ref-name: main
    capture-job-files: .github/actionspec-fragments

- name: Validate one workflow explicitly
  uses: v4lproik/github-actionspec-rs@main
  with:
    workflow: ci.yml
    actual: .github/actionspec-artifacts/ci-main.json
```

By convention, `validate-repo` mode defaults to:

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

- `mode`: action mode. Supported values are `capture` and `validate-repo`. Defaults to `validate-repo`
- `repo`: target repository root containing `.github/actionspec` declarations. Defaults to `.`
- `workflow`: workflow file name to capture or validate. Optional for `validate-repo` when the provided payloads all belong to the same workflow
- `ref-name`: optional workflow ref recorded in `capture` mode
- `capture-job-files`: one job fragment JSON file, a directory, a glob pattern, or a newline-separated list of those inputs for `capture` mode. Defaults to `.github/actionspec-fragments`
- `capture-inputs`: optional newline-separated list of `KEY=VALUE` workflow inputs recorded in `capture` mode
- `capture-file`: path where the action writes the normalized workflow payload in `capture` mode. Defaults to `/github/runner_temp/github-actionspec-capture/current/workflow-run.json`
- `actual`: path to one normalized workflow run JSON payload, a directory containing JSON payloads, a glob pattern, or a newline-separated list of payloads and glob patterns. Defaults to `.github/actionspec-artifacts`
- `declarations-dir`: custom declarations directory. Defaults to `.github/actionspec`
- `report-file`: path where the action writes the JSON validation report. Defaults to `/github/runner_temp/github-actionspec-dashboard/current/validation-report.json`
- `baseline-report`: optional path to a previous JSON validation report used to compute matrix diffs
- `dashboard-file`: path where the action writes the markdown matrix dashboard. Defaults to `/github/runner_temp/github-actionspec-dashboard/current/dashboard.md`
- `dashboard-output-keys`: optional newline-separated list of output keys to include in the dashboard and PR comment
- `write-summary`: whether to append the matrix dashboard to the job summary. Defaults to `true`
- `comment-pr`: whether to upsert a PR comment containing a short validation summary and the full matrix dashboard. Defaults to `false`
- `comment-title`: title used for the PR comment. Defaults to `Workflow Matrix Dashboard`
- `comment-tag`: stable marker used to find and update the existing PR comment. Defaults to `github-actionspec-matrix`
- `github-token`: token used for PR comment upserts when `comment-pr` is enabled

Examples:

```yaml
- name: Capture the workflow payload from job fragments
  id: actionspec-capture
  uses: v4lproik/github-actionspec-rs@main
  with:
    mode: capture
    workflow: ci.yml
    ref-name: ${{ github.ref_name }}
    capture-inputs: |
      run_ci=true
      run_pages=false
    capture-job-files: |
      .github/actionspec-fragments/build.json
      .github/actionspec-fragments/tests.json

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

- name: Validate matrix payloads for a build workflow
  uses: v4lproik/github-actionspec-rs@main
  with:
    repo: .
    workflow: build.yml
    actual: |
      .github/actionspec-artifacts/build-ts-service.json
      .github/actionspec-artifacts/build-rust-service.json
    dashboard-output-keys: |
      contract_build
      artifact_name

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

The capture mode writes `capture-path` to `GITHUB_OUTPUT`. The validate mode writes `report-path` and `dashboard-path`.

To show the difference between the current and previous matrix, download the earlier report artifact before the action step and pass its report JSON through `baseline-report`. The action updates a single PR comment identified by `comment-tag`, with a short status summary followed by the full matrix, so the discussion stays in one place instead of growing a new comment on every push.

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
