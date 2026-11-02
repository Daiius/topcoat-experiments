import Image from "next/image";

import { type Product, initials } from "@/lib/catalog";
import { IMAGE_SIZE, productImage } from "@/lib/images";

/**
 * A product's image, resized by `next/image` to what `sizes` asks for.
 * `frame` sizes the box; the image covers it. Falls back to the initials
 * placeholder for a category without images.
 */
export function ProductPhoto({
  product,
  frame,
  sizes,
  preload = false,
  eager = false,
}: {
  product: Product;
  frame: string;
  /** Rendered width at each breakpoint; picks the `srcset` candidate. */
  sizes: string;
  /** Adds a `<link rel="preload">` for it. */
  preload?: boolean;
  /** `loading="eager"` with high fetch priority, for a likely LCP image. */
  eager?: boolean;
}) {
  const src = productImage(product);
  if (src === null) {
    return (
      <div
        className={`${frame} flex items-center justify-center bg-slate-100 text-3xl font-bold text-slate-300`}
      >
        {initials(product.name)}
      </div>
    );
  }
  return (
    <Image
      src={src}
      alt={product.name}
      width={IMAGE_SIZE}
      height={IMAGE_SIZE}
      sizes={sizes}
      // The docs say not to combine `preload` with `loading`/`fetchPriority`.
      preload={preload}
      loading={eager && !preload ? "eager" : undefined}
      fetchPriority={eager && !preload ? "high" : undefined}
      className={`${frame} w-full bg-slate-100 object-cover`}
    />
  );
}
