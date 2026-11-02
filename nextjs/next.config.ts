import type { NextConfig } from "next";

// The shared data in ../data is copied into this package by
// scripts/copy-data.mjs (predev / prebuild), so Turbopack's root and the
// output file tracing root stay at their defaults (this directory).
const nextConfig: NextConfig = {
  cacheComponents: true,
  partialPrefetching: true,
  // Only for the local docker measurements; Vercel builds without it.
  output: process.env.STANDALONE ? "standalone" : undefined,
  images: {
    // Product images, copied from ../data/images by scripts/copy-data.mjs.
    localPatterns: [{ pathname: "/images/**", search: "" }],
    qualities: [75],
  },
};

export default nextConfig;
