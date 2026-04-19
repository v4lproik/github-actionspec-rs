# Contributing

This repository keeps the command surface centralized through `just`.

## Local Setup

- Install host tools with `just install`.
- Use Docker-backed `just` commands for build, lint, test, and coverage.
- Use `mise` only for host-side tool management and host-side commands exposed through `just`.

Main commands:

```bash
just docker-build
just docker-build-runtime
just fmt
just build
just lint
just test
just ci
just coverage
just coverage-summary
just coverage-ci
just discover
just validate-callers .
just validate-repo . ci.yml tests/fixtures/ci/baseline.json
```

## Command Rules

- Prefer `just` over open-coded `cargo`, `gh`, or `mise exec` commands in workflows, scripts, and docs.
- Keep `.mise.toml` limited to tool and version management.
- Keep `docker-bake.hcl` as the source of truth for Docker targets.
- Keep `justfile` as the canonical command interface.

## Validation Expectations

Before pushing code, the repository standard is:

```bash
just fmt
just lint
just test
```

If Docker is unavailable locally, note that explicitly when reporting verification status because the canonical repository checks are Docker-backed.

## Docs

- `README.md` is the short product entrypoint.
- `docs/` holds user-facing product docs.
- Repository-specific CI behavior belongs in `docs/ci-reference.html`, not in the README.

## Pull Requests

- Branch names should use `<github_nickname>/<github_issue>-<title_name>`.
- PR titles should include the GitHub issue number.
- Commit messages should follow `<type>(<scope>): <message>`.

Examples:

- `feat(cli): add runtime issue summary`
- `docs(site): add troubleshooting page`
