set shell := ["bash", "-euo", "pipefail", "-c"]

default:
  @just --list

install:
  mise install

fmt:
  mise exec -- cargo fmt

fmt-check:
  mise exec -- cargo fmt --check

build:
  mise exec -- cargo build --locked

check:
  mise exec -- cargo check --locked

clippy:
  mise exec -- cargo clippy --all-targets --all-features --locked -- -D warnings

lint: fmt-check clippy

test:
  mise exec -- cargo test --locked

coverage:
  mise exec -- cargo llvm-cov --all-features --workspace --html

coverage-summary:
  mise exec -- cargo llvm-cov --all-features --workspace --summary-only

coverage-ci:
  # `cargo llvm-cov --output-path` does not create the parent directory for us.
  mkdir -p target/llvm-cov
  mise exec -- cargo llvm-cov --all-features --workspace --lcov --output-path target/llvm-cov/lcov.info

ci: build lint test

pr-create base="main":
  gh pr create --base {{base}} --fill

pr-create-draft base="main":
  gh pr create --base {{base}} --fill --draft

discover repo="/Users/v4lproik/Programmation/dwarves/factmachine-monorepo":
  mise exec -- cargo run -- discover --repo {{repo}}

validate-repo repo workflow actual:
  mise exec -- cargo run -- validate-repo --repo {{repo}} --workflow {{workflow}} --actual {{actual}}

validate schema contract actual:
  mise exec -- cargo run -- validate --schema {{schema}} --contract {{contract}} --actual {{actual}}
