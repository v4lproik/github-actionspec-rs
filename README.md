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
just validate-repo /path/to/repo build-infrastructure.yml /path/to/actual.json
```

Commands:

- `github-actionspec discover --repo <path>`
- `github-actionspec validate --schema <file> --schema <file> --contract <file> --actual <file>`
- `github-actionspec validate-repo --repo <path> --workflow <name> --actual <file>`

## GitHub Action

This repository also exposes a Docker-based GitHub Action for the common `validate-repo` flow. The action runs the bundled `github-actionspec` binary together with the bundled `cue` runtime, so the calling workflow only needs a checked out repository and a normalized JSON payload.

```yaml
- uses: actions/checkout@v6

- name: Validate build-infrastructure contract
  uses: v4lproik/github-actionspec-rs@main
  with:
    repo: .
    workflow: build-infrastructure.yml
    actual: .github/actionspec-artifacts/build-infrastructure.json
```

Inputs:

- `repo`: target repository root containing `.github/actionspec` declarations. Defaults to `.`
- `workflow`: workflow file name to validate
- `actual`: path to one normalized workflow run JSON payload, a directory containing JSON payloads, or a newline-separated list of payload paths
- `declarations-dir`: custom declarations directory. Defaults to `.github/actionspec`

Examples:

```yaml
- name: Validate one payload
  uses: v4lproik/github-actionspec-rs@main
  with:
    repo: .
    workflow: build-infrastructure.yml
    actual: .github/actionspec-artifacts/build-infrastructure.json

- name: Validate a whole folder of payloads
  uses: v4lproik/github-actionspec-rs@main
  with:
    repo: .
    workflow: build-infrastructure.yml
    actual: .github/actionspec-artifacts/passing

- name: Validate an explicit list of payloads
  uses: v4lproik/github-actionspec-rs@main
  with:
    repo: .
    workflow: build-infrastructure.yml
    actual: |
      .github/actionspec-artifacts/staging.json
      .github/actionspec-artifacts/production.json
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
- Build, lint, test, local action smoke, coverage, runtime verification, and publish only run when `core` or `infra` changed.
- The workflow can also be started manually through `workflow_dispatch`; manual runs force the full CI path even if no matching file changes are present.
- Build: `just build`
- Lint: `just lint`
- Test: `just test`
- Coverage upload: `just coverage-ci`
- Local full pass: `just ci`
- `CI Test` is a separate workflow that checks out the fixture repository data from this repo and validates the published `v4lproik/github-actionspec-rs@main` action reference end to end.

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
