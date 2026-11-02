#!/usr/bin/env python3
"""Generate the synthetic product images in data/images/.

Both storefronts share these files. They are deliberately large originals
(1600x1600 JPEG) so that image optimization, or the lack of it, shows up in
what each framework delivers.

Assignment rule (both versions must follow it):

    data/images/{category_slug}-{(product_id % VARIANTS) + 1}.jpg

Output is deterministic: the same script produces byte-identical files.

Usage: python3 scripts/gen_images.py [--check]
"""

import argparse
import hashlib
import json
import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "data" / "images"
SIZE = 1600
VARIANTS = 3
QUALITY = 88

# Base hue (RGB) per category, picked to tell categories apart at a glance.
PALETTE = {
    "audio": (79, 70, 229),
    "displays": (14, 116, 144),
    "peripherals": (190, 24, 93),
    "storage": (180, 83, 9),
    "networking": (21, 128, 61),
    "cameras": (55, 65, 81),
    "wearables": (219, 39, 119),
    "smart-home": (8, 145, 178),
}


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def render(slug: str, variant: int) -> Image.Image:
    seed = int.from_bytes(hashlib.sha256(f"{slug}-{variant}".encode()).digest()[:8], "big")
    rng = random.Random(seed)
    base = PALETTE[slug]
    light = mix(base, (255, 255, 255), 0.85)
    dark = mix(base, (0, 0, 0), 0.35)

    # Vertical gradient backdrop.
    img = Image.new("RGB", (SIZE, SIZE))
    draw = ImageDraw.Draw(img)
    for y in range(SIZE):
        draw.line([(0, y), (SIZE, y)], fill=mix(light, mix(base, light, 0.5), y / SIZE))

    # Soft floor shadow.
    shadow = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(shadow).ellipse((380, 1240, 1220, 1380), fill=110)
    shadow = shadow.filter(ImageFilter.GaussianBlur(40))
    img.paste(dark, (0, 0), shadow)

    # The "product": a rounded body plus a few details, varied per variant.
    w = rng.randint(620, 900)
    h = rng.randint(560, 900)
    x0 = (SIZE - w) // 2
    y0 = 1300 - h
    radius = rng.choice([40, 120, 260])
    draw.rounded_rectangle((x0, y0, x0 + w, y0 + h), radius=radius, fill=base)
    draw.rounded_rectangle(
        (x0 + 40, y0 + 40, x0 + w - 40, y0 + h // 2), radius=max(radius - 30, 10), fill=dark
    )
    for _ in range(rng.randint(2, 5)):
        r = rng.randint(30, 110)
        cx = rng.randint(x0 + r + 20, x0 + w - r - 20)
        cy = rng.randint(y0 + h // 2 + r // 2, y0 + h - r - 10)
        draw.ellipse((cx - r, cy - r, cx + r, cy + r), fill=light, outline=dark, width=12)

    # Specular highlight.
    glow = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(glow).ellipse((x0 + 60, y0 + 50, x0 + w // 2, y0 + 160), fill=90)
    glow = glow.filter(ImageFilter.GaussianBlur(30))
    img.paste((255, 255, 255), (0, 0), glow)

    # Fine grain so the JPEG has photo-like weight instead of flat colour.
    # Seeded rather than Image.effect_noise, whose C RNG is not reproducible.
    noise = Image.frombytes("L", (SIZE, SIZE), rng.randbytes(SIZE * SIZE)).convert("RGB")
    return Image.blend(img, noise, 0.04)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="list files without writing")
    args = parser.parse_args()

    slugs = [c["slug"] for c in json.loads((ROOT / "data" / "products.json").read_text())["categories"]]
    missing = set(slugs) - PALETTE.keys()
    if missing:
        print(f"no palette for categories: {sorted(missing)}", file=sys.stderr)
        return 1

    OUT.mkdir(parents=True, exist_ok=True)
    for slug in slugs:
        for variant in range(1, VARIANTS + 1):
            path = OUT / f"{slug}-{variant}.jpg"
            if args.check:
                print(path.relative_to(ROOT), "exists" if path.exists() else "missing")
                continue
            render(slug, variant).save(path, "JPEG", quality=QUALITY, optimize=True)
            print(path.relative_to(ROOT), path.stat().st_size)
    return 0


if __name__ == "__main__":
    sys.exit(main())
