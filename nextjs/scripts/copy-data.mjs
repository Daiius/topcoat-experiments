// Copies the shared data from the repository root into this package, before
// dev, build, typecheck, and lint:
//
// - ../data/products.json -> src/generated/products.json (imported by the
//   catalog; Turbopack does not resolve files outside the package root)
// - ../data/images/*.jpg  -> public/images/ (served and optimized by next/image)
//
// Copies, not symlinks, and both targets are gitignored: ../data stays the
// single source of truth.
import { cpSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import path from "node:path";

const here = import.meta.dirname;
const data = path.join(here, "..", "..", "data");
const generated = path.join(here, "..", "src", "generated");
const images = path.join(here, "..", "public", "images");

mkdirSync(generated, { recursive: true });
cpSync(path.join(data, "products.json"), path.join(generated, "products.json"));

rmSync(images, { recursive: true, force: true });
mkdirSync(images, { recursive: true });
const files = readdirSync(path.join(data, "images")).filter((name) => name.endsWith(".jpg"));
for (const name of files) {
  cpSync(path.join(data, "images", name), path.join(images, name));
}

console.log(`copy-data: products.json -> src/generated, ${files.length} images -> public/images`);
