FROM rust:1.84.1-bookworm AS dev

ARG CARGO_LLVM_COV_VERSION=0.6.15

# Keep the image focused on the repo's build and verification toolchain so local
# development and CI execute against the same Rust environment.
RUN rustup component add clippy rustfmt llvm-tools \
    && cargo install --locked cargo-llvm-cov --version "${CARGO_LLVM_COV_VERSION}"

WORKDIR /workspace

FROM rust:1.84.1-bookworm AS runtime-builder

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY schema ./schema
COPY src ./src
RUN cargo build --locked --release

FROM golang:1.24-bookworm AS cue-builder

ARG CUE_VERSION=v0.15.0
RUN GOBIN=/cue-bin go install cuelang.org/go/cmd/cue@${CUE_VERSION}

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
# The binary resolves bundled schemas relative to CARGO_MANIFEST_DIR, so the runtime image
# must preserve the schema directory at /workspace/schema.
COPY --from=runtime-builder /workspace/schema ./schema
COPY --from=runtime-builder /workspace/target/release/github-actionspec /usr/local/bin/github-actionspec
COPY --from=cue-builder /cue-bin/cue /usr/local/bin/cue

ENTRYPOINT ["github-actionspec"]
