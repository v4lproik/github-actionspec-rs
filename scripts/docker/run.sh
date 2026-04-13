#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cache_root="${repo_root}/.docker-cache/cargo"
image="${GITHUB_ACTIONSPEC_DOCKER_IMAGE:-github-actionspec-rs-dev:local}"

mkdir -p "${cache_root}"

# Run the container as the current host user so build artifacts and coverage output stay
# writable from the local checkout and from CI workspaces.
docker run --rm \
  --user "$(id -u):$(id -g)" \
  --workdir /workspace \
  --env HOME=/tmp/github-actionspec-home \
  --env CARGO_HOME=/cargo-home \
  --env CARGO_TARGET_DIR=/workspace/target \
  --volume "${repo_root}:/workspace" \
  --volume "${cache_root}:/cargo-home" \
  "${image}" \
  "$@"
