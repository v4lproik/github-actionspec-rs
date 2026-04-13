# AGENTS.md

## Purpose
- Rust implementation of `github-actionspec`.
- Validates GitHub Actions workflow contracts expressed in `CUE`.
- Target repository declarations live under `.github/actionspec/**/*.cue`.

## Commands
- Use Docker-backed `just` recipes for build, lint, test, and coverage.
- Use `mise` for host-side tool management only.
- Use `just` for repository commands.
- Main commands:
  - `just install`
  - `just docker-build`
  - `just docker-build-runtime`
  - `just fmt`
  - `just build`
  - `just lint`
  - `just test`
  - `just ci`
  - `just coverage`
  - `just coverage-summary`
  - `just coverage-ci`
  - `just discover`
  - `just pr-create`
  - `just validate-repo /path/to/repo ci.yml /path/to/actual.json`

## Rules
- Keep `.mise.toml` minimal and limited to tool/version management.
- Keep the Docker image focused on build/test/lint/coverage parity between local development and CI.
- Keep `docker-bake.hcl` as the single place for Docker build target configuration.
- Keep `justfile` as the canonical command interface.
- Keep a dedicated runtime image target available for distributing the CLI with bundled schema files and the `cue` binary.
- Do not add raw `cargo`, `gh`, or `mise exec` command flows to docs, scripts, or workflows when a `just` recipe can own them.
- Expose new command flows through `just` first, then document them in `README.md`.
- Keep repository-owned declarations and fixtures self-contained inside this repository.
- `cue` is still an external runtime dependency and must be on `PATH` for host-side validation runs.
- Repository automation should invoke `just`, including CI jobs and PR creation flows.

## Git Conventions
- Branches/PR heads should be formatted as `<github_nickname>/<github_issue>-<title_name>`.
- PR titles should include the GitHub issue ticker number.
- Commit messages should follow `<type>(<scope>): <message>`, for example `feat(cli): add coverage export command`.

## Validation
- For code changes, run `just test`.
- For CLI or discovery changes, also run `just discover`.
- The default `just discover` target points to the current repository root.
