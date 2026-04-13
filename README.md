# github-actionspec-rs

Rust implementation of the GitHub Actions workflow contract validator.

It keeps CUE as the intermediate language and uses the `cue` CLI for validation.

## Tooling

This repo uses `mise` for local tool management and `just` for repository commands.

```bash
mise install
just test
just discover
```

The repo-local configuration lives in [.mise.toml](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/.mise.toml) and currently pins the Rust toolchain to `1.84.1`.

This repo also exposes the common commands through [justfile](/Users/v4lproik/Programmation/v4lproik/github-actionspec-rs/justfile):

```bash
just install
just test
just discover
just validate-repo /path/to/repo build-infrastructure.yml /path/to/actual.json
```

Commands:

- `github-actionspec discover --repo <path>`
- `github-actionspec validate --schema <file> --schema <file> --contract <file> --actual <file>`
- `github-actionspec validate-repo --repo <path> --workflow <name> --actual <file>`
