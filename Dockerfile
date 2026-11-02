# syntax=docker/dockerfile:1
#
# topcoat storefront. Vercel Container Runtime builds the `Dockerfile` at the
# project root, so this file lives here; the build context is the repository
# root because the crate reads ../data via `include_str!` and `asset!`.
#
#   DOCKER_BUILDKIT=1 docker build -t storefront-topcoat .
#   docker run --rm -p 8081:80 -e COOKIE_KEY="$(openssl rand -hex 64)" storefront-topcoat
#
# Network access is needed at build time: crates.io, and the Tailwind CLI
# binary that build.rs downloads from GitHub releases.

ARG RUST_VERSION=1.99
ARG TOPCOAT_CLI_VERSION=0.11.0

FROM rust:${RUST_VERSION}-bookworm AS builder
ARG TOPCOAT_CLI_VERSION

# `topcoat asset bundle` lives in topcoat-cli. Its own target dir is a cache
# mount, so a rebuild of this layer recompiles only what changed.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/var/cache/topcoat-cli-target,sharing=locked \
    cargo install topcoat-cli --version "${TOPCOAT_CLI_VERSION}" --locked \
        --target-dir /var/cache/topcoat-cli-target

WORKDIR /src
COPY data ./data
COPY topcoat ./topcoat
WORKDIR /src/topcoat

# target/ is a cache mount: dependencies, the downloaded Tailwind CLI and the
# asset bundler cache (both kept under target/) survive between builds.
# Cargo decides freshness by mtime, and a source restored from an older commit
# can be older than the cached artifacts, so touch the crate's own sources to
# always rebuild this crate (dependencies stay cached).
# Anything in the cache mount disappears with the layer, so the binary and the
# bundle are copied out to /out in the same RUN.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/src/topcoat/target,sharing=locked \
    set -eux; \
    find src build.rs -type f -exec touch {} +; \
    cargo build --release --locked; \
    mkdir -p /out; \
    topcoat asset bundle --release --out /out/assets; \
    cp target/release/storefront-topcoat /out/storefront-topcoat

# glibc + libgcc, no shell. Same base as Vercel's Rust guide.
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
# AssetBundle looks for `assets/` beside `current_exe()`, so both go to /app.
COPY --from=builder /out/ /app/

# topcoat::start binds HOST:PORT (default 127.0.0.1:3000). A loopback listener
# is unreachable from the platform proxy, so bind every interface.
# PORT=80 follows Vercel's Rust guide: Vercel routes to port 80 unless PORT is
# set in the project settings. Docker (20.10+) sets
# net.ipv4.ip_unprivileged_port_start=0 in the container, so the nonroot user
# can bind 80. bench/compose.yml overrides PORT=8080 for both apps.
ENV HOST=0.0.0.0 \
    PORT=80
EXPOSE 80

# COOKIE_KEY (128 hex digits, shared by every instance) is required at run
# time in release builds and is never baked into the image.
# No HEALTHCHECK: distroless has no shell or curl. Probe from outside.
ENTRYPOINT ["/app/storefront-topcoat"]
