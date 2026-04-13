#!/usr/bin/env bash
set -euo pipefail

runtime_image="${GITHUB_ACTIONSPEC_RUNTIME_IMAGE:-github-actionspec-rs:local}"

docker run --rm "${runtime_image}" "$@"
