import path from "node:path";

import type { NextConfig } from "next";

// The catalog is imported from ../data/products.json, outside this package.
// Turbopack does not resolve files outside its root, and the root is
// detected from the lockfile here, so widen both roots to the repository.
const repositoryRoot = path.join(import.meta.dirname, "..");

const nextConfig: NextConfig = {
  cacheComponents: true,
  partialPrefetching: true,
  // Only for the local docker measurements; Vercel builds without it.
  output: process.env.STANDALONE ? "standalone" : undefined,
  turbopack: {
    root: repositoryRoot,
  },
  outputFileTracingRoot: repositoryRoot,
  images: {
    // Product images, copied from ../data/images by scripts/copy-images.mjs.
    localPatterns: [{ pathname: "/images/**", search: "" }],
    qualities: [75],
  },
};

export default nextConfig;
