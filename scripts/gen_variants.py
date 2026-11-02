#!/usr/bin/env python3
"""Pre-resize data/images/*.jpg into WebP variants for the topcoat version.

topcoat has no image optimizer, so the sizes a `srcset` can pick from are
made ahead of time here and committed. The Next.js version keeps using the
originals (next/image resizes them on request).

    data/images/variants/{stem}-{width}.webp   for width in WIDTHS

Quality 75 matches the Next.js version's `images.qualities`.

Usage: python3 scripts/gen_variants.py [--check]
"""

import argparse
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "data" / "images"
OUT = SRC / "variants"
# Keep in sync with `WIDTHS` in topcoat/src/images.rs.
WIDTHS = (320, 640, 960, 1600)
QUALITY = 75


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="list files without writing")
    args = parser.parse_args()

    originals = sorted(SRC.glob("*.jpg"))
    if not originals:
        print(f"no originals in {SRC}", file=sys.stderr)
        return 1

    OUT.mkdir(parents=True, exist_ok=True)
    for original in originals:
        with Image.open(original) as img:
            for width in WIDTHS:
                path = OUT / f"{original.stem}-{width}.webp"
                if args.check:
                    print(path.relative_to(ROOT), "exists" if path.exists() else "missing")
                    continue
                height = round(img.height * width / img.width)
                resized = img if width == img.width else img.resize((width, height), Image.LANCZOS)
                resized.save(path, "WEBP", quality=QUALITY, method=6)
                print(path.relative_to(ROOT), path.stat().st_size)
    return 0


if __name__ == "__main__":
    sys.exit(main())
