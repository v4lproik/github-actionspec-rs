set shell := ["bash", "-euo", "pipefail", "-c"]

docker-image := env_var_or_default("GITHUB_ACTIONSPEC_DOCKER_IMAGE", "github-actionspec-rs-dev:local")
runtime-image := env_var_or_default("GITHUB_ACTIONSPEC_RUNTIME_IMAGE", "github-actionspec-rs:local")
docker-runner := "./scripts/docker/run.sh"
docker-runtime-runner := "./scripts/docker/run-runtime.sh"
host-runner := "mise exec --"
cue-go-version := `version="$(sed -n 's/^"asdf:asdf-community\/asdf-cue" = "\(.*\)"$/\1/p' .mise.toml | head -n1)"; if [ -z "$version" ]; then echo "failed to resolve CUE version from .mise.toml" >&2; exit 1; fi; case "$version" in v*) printf '%s' "$version" ;; *) printf 'v%s' "$version" ;; esac`

default:
  @just --list

install:
  mise install

docker-build:
  CUE_VERSION={{cue-go-version}} IMAGE_TAG={{docker-image}} docker buildx bake --load dev

docker-build-runtime:
  CUE_VERSION={{cue-go-version}} RUNTIME_IMAGE_TAG={{runtime-image}} docker buildx bake --load runtime

docker-smoke-runtime:
  {{docker-runtime-runner}} --help

docker-run-runtime:
  just docker-build-runtime
  just docker-smoke-runtime

docker-push-runtime image="docker.io/valproik/github-actionspec-rs:latest":
  CUE_VERSION={{cue-go-version}} RUNTIME_IMAGE_TAG={{image}} docker buildx bake --push runtime

runtime-ci:
  just docker-build-runtime
  just docker-smoke-runtime

fmt:
  just docker-build
  {{docker-runner}} cargo fmt

fmt-check:
  just docker-build
  {{docker-runner}} cargo fmt --check

build:
  just docker-build
  {{docker-runner}} cargo build --locked

check:
  just docker-build
  {{docker-runner}} cargo check --locked

clippy:
  just docker-build
  {{docker-runner}} cargo clippy --all-targets --all-features --locked -- -D warnings

lint:
  just docker-build
  {{docker-runner}} cargo fmt --check
  {{docker-runner}} cargo clippy --all-targets --all-features --locked -- -D warnings
  {{docker-runner}} cargo run -- validate-callers --repo .

test:
  just docker-build
  {{docker-runner}} cargo test --locked

coverage:
  just docker-build
  {{docker-runner}} cargo llvm-cov --all-features --workspace --html

coverage-summary:
  just docker-build
  {{docker-runner}} cargo llvm-cov --all-features --workspace --summary-only

coverage-ci:
  just docker-build
  # `cargo llvm-cov --output-path` does not create the parent directory for us.
  mkdir -p target/llvm-cov
  {{docker-runner}} cargo llvm-cov --all-features --workspace --lcov --output-path target/llvm-cov/lcov.info

ci:
  just docker-build
  {{docker-runner}} cargo build --locked
  {{docker-runner}} cargo fmt --check
  {{docker-runner}} cargo clippy --all-targets --all-features --locked -- -D warnings
  {{docker-runner}} cargo run -- validate-callers --repo .
  {{docker-runner}} cargo test --locked

pr-create base="main":
  {{host-runner}} gh pr create --base {{base}} --fill

pr-create-draft base="main":
  {{host-runner}} gh pr create --base {{base}} --fill --draft

discover repo=".":
  {{host-runner}} cargo run -- discover --repo {{repo}}

emit-fragment job result file:
  {{host-runner}} cargo run -- emit-fragment --job {{job}} --result {{result}} --file {{file}}

capture workflow output job_file ref="":
  if [ -n "{{ref}}" ]; then {{host-runner}} cargo run -- capture --workflow {{workflow}} --ref {{ref}} --job-file {{job_file}} --output {{output}}; else {{host-runner}} cargo run -- capture --workflow {{workflow}} --job-file {{job_file}} --output {{output}}; fi

validate-callers repo=".":
  {{host-runner}} cargo run -- validate-callers --repo {{repo}}

validate-callers-report repo report:
  {{host-runner}} cargo run -- validate-callers --repo {{repo}} --report-file {{report}} --dry-run

validate-repo repo workflow actual:
  {{host-runner}} cargo run -- validate-repo --repo {{repo}} --workflow {{workflow}} --actual {{actual}}

validate-repo-report repo workflow actual report:
  just docker-build
  {{docker-runner}} cargo run -- validate-repo --repo {{repo}} --workflow {{workflow}} --actual {{actual}} --report-file {{report}}

validate-repo-report-dry repo workflow actual report:
  just docker-build
  {{docker-runner}} cargo run -- validate-repo --repo {{repo}} --workflow {{workflow}} --actual {{actual}} --report-file {{report}} --dry-run

validate-repo-dashboard repo workflow actual report dashboard baseline="" output_keys="" dry="false":
  just docker-build
  status=0; dry_arg=""; if [ "{{dry}}" = "true" ]; then dry_arg=" --dry-run"; fi; if ! eval "{{docker-runner}} cargo run -- validate-repo --repo {{repo}} --workflow {{workflow}} --actual {{actual}} --report-file {{report}}$dry_arg"; then status=$?; fi; if [ -f "{{report}}" ]; then output_key_args="$(printf '%s\n' '{{output_keys}}' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//' | awk 'NF {printf " --output-key %s", $0}')"; if [ -n "{{baseline}}" ]; then eval "{{docker-runner}} cargo run -- dashboard --current {{report}} --baseline {{baseline}} --output {{dashboard}}$output_key_args"; else eval "{{docker-runner}} cargo run -- dashboard --current {{report}} --output {{dashboard}}$output_key_args"; fi; fi; exit "$status"

dashboard-report current output baseline="" output_keys="":
  just docker-build
  output_key_args="$(printf '%s\n' '{{output_keys}}' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//' | awk 'NF {printf " --output-key %s", $0}')"; if [ -n "{{baseline}}" ]; then eval "{{docker-runner}} cargo run -- dashboard --current {{current}} --baseline {{baseline}} --output {{output}}$output_key_args"; else eval "{{docker-runner}} cargo run -- dashboard --current {{current}} --output {{output}}$output_key_args"; fi

validate schema contract actual:
  {{host-runner}} cargo run -- validate --schema {{schema}} --contract {{contract}} --actual {{actual}}
