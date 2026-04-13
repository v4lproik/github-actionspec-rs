# github-actionspec-rs

[![CI](https://github.com/v4lproik/github-actionspec-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/v4lproik/github-actionspec-rs/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/v4lproik/github-actionspec-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/v4lproik/github-actionspec-rs)

Rust implementation of the GitHub Actions workflow contract validator.

It keeps CUE as the intermediate language and uses the `cue` CLI for validation.

## Tooling

This repo uses `mise` for local tool management and `just` for repository commands.

```bash
mise install
just lint
just test
just ci
just discover
just coverage
just coverage-summary
```

The repo-local configuration lives in [.mise.toml](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/.mise.toml) and currently pins the Rust toolchain to `1.84.1`.

This repo also exposes the common commands through [justfile](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/justfile):

```bash
just install
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

## Coverage

The target for this repo is to stay close to `90%` test coverage.

Use:

```bash
just coverage
just coverage-summary
```

This runs `cargo llvm-cov` through `mise exec`. Use `just coverage` for the HTML report and `just coverage-summary` for the terminal summary. The `cargo-llvm-cov` subcommand must be available in the local Rust environment for real coverage runs.

For CI and Codecov uploads, use:

```bash
just coverage-ci
```

This emits `target/llvm-cov/lcov.info`, which the repository workflow uploads to Codecov.

## CI

GitHub Actions must call `just`, not raw `cargo`, `gh`, or `mise` command sequences.

- Build: `just build`
- Lint: `just lint`
- Test: `just test`
- Coverage upload: `just coverage-ci`
- Local full pass: `just ci`

## Pull Requests

Open repository PRs through `just` so the command surface stays centralized:

```bash
just pr-create
just pr-create-draft
```

Both recipes default the base branch to `main`.
