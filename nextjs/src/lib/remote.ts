// Stand-ins for slow upstream services (a reviews API, a recommendation
// engine), with the same delays as `topcoat/src/remote.rs`. Each call sleeps,
// then reads the bundled catalog.
//
// The two differ only in how they are cached, which is the point of #2:
// reviews must be fresh, so they run on every request and stream in;
// recommendations may be shared, so they are cached and baked into the
// static shell at build time.
import { cacheLife } from "next/cache";
import { connection } from "next/server";

import { type Product, type Review, getProduct, lookupProducts } from "./catalog";

/** Simulated latency of the reviews service, in milliseconds. */
export const REVIEWS_DELAY_MS = 200;

/** Simulated latency of the recommendation service, in milliseconds. */
export const RELATED_DELAY_MS = 800;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Fetches the reviews of a product, uncached. Unknown IDs have none. */
export async function fetchReviews(productId: number): Promise<Review[]> {
  // A real `fetch` is request-time data by itself. A timer is not obviously
  // so, so say it outright: this runs per request, never at build time.
  await connection();
  await sleep(REVIEWS_DELAY_MS);
  return (await getProduct(productId))?.reviews ?? [];
}

/**
 * Fetches the products recommended alongside a product. Cached for as long
 * as the catalog, so the 800ms is paid once per product at build time.
 */
export async function fetchRelated(productId: number): Promise<Product[]> {
  "use cache";
  cacheLife("max");
  await sleep(RELATED_DELAY_MS);
  const product = await getProduct(productId);
  return product ? lookupProducts(product.related_ids) : [];
}
