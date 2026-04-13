set shell := ["bash", "-euo", "pipefail", "-c"]

docker-image := env_var_or_default("GITHUB_ACTIONSPEC_DOCKER_IMAGE", "github-actionspec-rs-dev:local")
runtime-image := env_var_or_default("GITHUB_ACTIONSPEC_RUNTIME_IMAGE", "github-actionspec-rs:local")
docker-runner := "./scripts/docker/run.sh"
docker-runtime-runner := "./scripts/docker/run-runtime.sh"
host-runner := "mise exec --"

default:
  @just --list

install:
  mise install

docker-build:
  IMAGE_TAG={{docker-image}} docker buildx bake --load dev

docker-build-runtime:
  RUNTIME_IMAGE_TAG={{runtime-image}} docker buildx bake --load runtime

docker-smoke-runtime:
  {{docker-runtime-runner}} --help

docker-run-runtime:
  just docker-build-runtime
  just docker-smoke-runtime

docker-push-runtime image="docker.io/valproik/github-actionspec-rs:latest":
  RUNTIME_IMAGE_TAG={{image}} docker buildx bake --push runtime

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
  {{docker-runner}} cargo test --locked

pr-create base="main":
  {{host-runner}} gh pr create --base {{base}} --fill

pr-create-draft base="main":
  {{host-runner}} gh pr create --base {{base}} --fill --draft

discover repo=".":
  {{host-runner}} cargo run -- discover --repo {{repo}}

validate-repo repo workflow actual:
  {{host-runner}} cargo run -- validate-repo --repo {{repo}} --workflow {{workflow}} --actual {{actual}}

validate schema contract actual:
  {{host-runner}} cargo run -- validate --schema {{schema}} --contract {{contract}} --actual {{actual}}
