import Link from "next/link";

import { type Product, formatPrice, formatRating } from "@/lib/catalog";

import { ProductPhoto } from "./product-photo";
import { RatingStars } from "./rating-stars";

/**
 * Image widths for the 2 / 3 / 4 column product grids inside the 72rem
 * container (a card is at most about 262px wide; the image is narrower).
 */
export const GRID_IMAGE_SIZES = "(min-width: 1024px) 262px, (min-width: 768px) 33vw, 50vw";

export function ProductCard({
  product,
  sizes = GRID_IMAGE_SIZES,
  preload = false,
}: {
  product: Product;
  sizes?: string;
  preload?: boolean;
}) {
  return (
    <Link
      href={`/products/${product.id}`}
      className="group flex flex-col rounded-xl border border-slate-200 bg-white p-4 shadow-sm transition hover:shadow-md"
    >
      <ProductPhoto product={product} frame="mb-4 h-32 rounded-lg" sizes={sizes} preload={preload} />
      <p className="text-xs font-medium uppercase tracking-wide text-slate-400">
        {product.category}
      </p>
      <h3 className="mt-1 text-sm font-semibold text-slate-900 group-hover:text-indigo-600">
        {product.name}
      </h3>
      <div className="mt-2 flex items-center gap-1">
        <RatingStars tenths={product.rating_tenths} size="h-4 w-4" />
        <span className="text-xs text-slate-500">
          {`${formatRating(product.rating_tenths)} (${product.review_count})`}
        </span>
      </div>
      <p className="mt-3 text-lg font-bold">{formatPrice(product.price_cents)}</p>
    </Link>
  );
}
