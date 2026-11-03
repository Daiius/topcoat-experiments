# The image Vercel's Container Runtime deploys for the topcoat project.
#
# Vercel builds the `Dockerfile` at the project root but keeps no build cache
# between container builds, so compiling topcoat-cli and the app there took
# 10 minutes on every deploy. Instead, build `topcoat.Dockerfile` locally
# (scripts/image-topcoat.sh tags it `:latest` in the local docker) and deploy
# a prebuilt output, so Vercel never builds:
#
#   ./scripts/image-topcoat.sh
#   vercel build --prod && vercel deploy --prebuilt --prod
#
# `vercel build` builds this file with the local docker, so `FROM` resolves to
# the local image; the result is pushed to Vercel Container Registry.
FROM ghcr.io/daiius/topcoat-experiments-topcoat:latest
