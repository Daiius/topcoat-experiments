# The image Vercel's Container Runtime deploys for the topcoat project.
#
# Vercel builds the `Dockerfile` at the project root but keeps no build cache
# between container builds, so compiling topcoat-cli and the app there took
# 10 minutes on every deploy. Instead, GitHub Actions builds
# `topcoat.Dockerfile` with a layer cache, pushes it to ghcr.io, and then runs
# `vercel deploy` with this line pinned to the pushed digest
# (.github/workflows/topcoat.yml). The committed `:latest` is only a fallback.
FROM ghcr.io/daiius/topcoat-experiments-topcoat:latest
