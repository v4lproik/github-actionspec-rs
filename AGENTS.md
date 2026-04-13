# AGENTS.md

## Purpose
- Rust implementation of `github-actionspec`.
- Validates GitHub Actions workflow contracts expressed in `CUE`.
- Target repository declarations live under `.github/actionspec/**/*.cue`.

## Commands
- Use `mise` for tool management only.
- Use `just` for repository commands.
- Main commands:
  - `just install`
  - `just build`
  - `just check`
  - `just test`
  - `just discover`
  - `just validate-repo /path/to/repo build-infrastructure.yml /path/to/actual.json`

## Rules
- Keep `.mise.toml` minimal and limited to tool/version management.
- Keep `justfile` as the canonical command interface.
- Expose new command flows through `just` and document them in `README.md`.
- Preserve compatibility with repository-owned `.cue` declarations already used in `factmachine-monorepo`.
- `cue` is still an external runtime dependency and must be on `PATH` for real validation runs.

## Git Conventions
- Branches/PR heads should be formatted as `<github_nickname>/<github_issue>-<title_name>`.
- PR titles should include the GitHub issue ticker number.
- Commit messages should follow `<type>(<scope>): <message>`, for example `chore(domain): message`.

## Validation
- For code changes, run `just test`.
- For CLI or discovery changes, also run `just discover`.
- The default `just discover` target points to `/Users/v4lproik/Programmation/dwarves/factmachine-monorepo`.
