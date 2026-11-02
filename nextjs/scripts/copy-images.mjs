// Copies the shared product images (../data/images) into public/images so
// next/image can serve and optimize them. A copy, not a symlink: public/images
// is gitignored, and symlinks there are easy to commit or to lose in a deploy.
import { cpSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import path from "node:path";

const here = import.meta.dirname;
const source = path.join(here, "..", "..", "data", "images");
const target = path.join(here, "..", "public", "images");

rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });

const files = readdirSync(source).filter((name) => name.endsWith(".jpg"));
for (const name of files) {
  cpSync(path.join(source, name), path.join(target, name));
}

console.log(`copy-images: ${files.length} files -> ${path.relative(process.cwd(), target)}`);
