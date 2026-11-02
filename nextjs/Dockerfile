# syntax=docker/dockerfile:1
#
# Next.js storefront, standalone output. Only for the local docker
# measurements: Vercel builds Next.js natively and never uses this file.
# The build context is the repository root, because scripts/copy-data.mjs
# copies ../data into the package before the build:
#
#   DOCKER_BUILDKIT=1 docker build -f nextjs/Dockerfile -t storefront-nextjs .
#   docker run --rm -p 8082:8080 storefront-nextjs

ARG NODE_VERSION=24

FROM node:${NODE_VERSION}-bookworm-slim AS builder
ENV NEXT_TELEMETRY_DISABLED=1 \
    COREPACK_ENABLE_DOWNLOAD_PROMPT=0
# corepack fetches the pnpm pinned in package.json's `packageManager`.
RUN corepack enable

WORKDIR /repo/nextjs
COPY nextjs/package.json nextjs/pnpm-lock.yaml nextjs/pnpm-workspace.yaml ./
# The pnpm store is a cache mount; it lives on another filesystem, so pnpm
# copies into node_modules instead of hard-linking.
RUN --mount=type=cache,target=/root/.cache/node/corepack \
    --mount=type=cache,target=/pnpm/store \
    pnpm install --frozen-lockfile --store-dir /pnpm/store

COPY data /repo/data
COPY nextjs ./
# copy-data also runs as `prebuild`; calling it explicitly does not depend on
# pnpm's pre/post script setting.
RUN --mount=type=cache,target=/root/.cache/node/corepack \
    pnpm run copy-data && STANDALONE=1 pnpm build

# Assemble the standalone server: Next.js leaves static files and public/ for
# the deployer to copy (they are meant for a CDN).
RUN set -eux; \
    mkdir -p /out; \
    cp -a .next/standalone/. /out/; \
    mkdir -p /out/.next; \
    cp -a .next/static /out/.next/static; \
    cp -a public /out/public

FROM gcr.io/distroless/nodejs24-debian12:nonroot
WORKDIR /app
# nonroot (65532) must own .next: next/image writes its cache to .next/cache.
COPY --from=builder --chown=65532:65532 /out/ /app/
ENV NODE_ENV=production \
    NEXT_TELEMETRY_DISABLED=1 \
    HOSTNAME=0.0.0.0 \
    PORT=8080
EXPOSE 8080
# The image's ENTRYPOINT is node itself. No HEALTHCHECK: no shell or curl.
CMD ["server.js"]
