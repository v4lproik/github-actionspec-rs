set shell := ["bash", "-euo", "pipefail", "-c"]

default:
  @just --list

install:
  mise install

build:
  mise exec -- cargo build

check:
  mise exec -- cargo check

test:
  mise exec -- cargo test

discover repo="/Users/v4lproik/Programmation/dwarves/factmachine-monorepo":
  mise exec -- cargo run -- discover --repo {{repo}}

validate-repo repo workflow actual:
  mise exec -- cargo run -- validate-repo --repo {{repo}} --workflow {{workflow}} --actual {{actual}}

validate schema contract actual:
  mise exec -- cargo run -- validate --schema {{schema}} --contract {{contract}} --actual {{actual}}
