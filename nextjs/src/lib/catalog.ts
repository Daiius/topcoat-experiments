import { cacheLife } from "next/cache";

// Copied from ../data/products.json by scripts/copy-data.mjs.
import data from "@/generated/products.json";

export interface Category {
  name: string;
  slug: string;
}

export interface Spec {
  key: string;
  value: string;
}

export interface Review {
  author: string;
  date: string;
  rating_tenths: number;
  title: string;
  body: string;
}

export interface Product {
  id: number;
  name: string;
  slug: string;
  category: string;
  category_slug: string;
  price_cents: number;
  rating_tenths: number;
  review_count: number;
  featured: boolean;
  specs: Spec[];
  description: string[];
  reviews: Review[];
  related_ids: number[];
}

/** Number of products rendered per `/products` page. */
export const PAGE_SIZE = 24;

// The bundled catalog. A module import is "predictable" to Cache Components:
// reading it needs no cache directive to be prerendered. The exported reads
// below still use `use cache` so their results (not just this module) are
// reused across requests and can join prefetches when given URL data.
const categories = data.categories as Category[];
const products = data.products as Product[];
const byId = new Map(products.map((product) => [product.id, product]));

export function getCategories(): Category[] {
  return categories;
}

/** Every product ID, for `generateStaticParams`. */
export function getProductIds(): number[] {
  return products.map((product) => product.id);
}

/** The product with `id`, or `null` (cached values must be serializable). */
export async function getProduct(id: number): Promise<Product | null> {
  "use cache";
  cacheLife("max");
  return byId.get(id) ?? null;
}

/** Whether `id` names a product. A plain map lookup, for validating input. */
export function isProductId(id: unknown): id is number {
  return typeof id === "number" && byId.has(id);
}

/** Products by ID, skipping unknown ones. A plain map lookup, not cached. */
export function lookupProducts(ids: number[]): Product[] {
  return ids.map((id) => byId.get(id)).filter((product) => product !== undefined);
}

export async function getFeatured(): Promise<Product[]> {
  "use cache";
  cacheLife("max");
  return products.filter((product) => product.featured);
}

export interface ProductsPage {
  items: Product[];
  current: number;
  pageCount: number;
  total: number;
}

/**
 * A page of products after filtering and sorting, with the same contract as
 * the topcoat version (`Catalog::page`): `category` matches an exact slug,
 * `search` (already normalized) keeps names containing it ignoring case,
 * `sort` must already be normalized, every sort breaks ties by ascending ID,
 * and `page` is clamped to the valid range.
 *
 * The arguments are the cache key, so each distinct URL is computed once per
 * instance and then served from the cache.
 */
export async function getProductsPage(
  page: number,
  sort: string | null,
  category: string | null,
  search: string | null,
): Promise<ProductsPage> {
  "use cache";
  cacheLife("max");

  const needle = search?.toLowerCase() ?? null;
  const items = products.filter(
    (product) =>
      (category === null || product.category_slug === category) &&
      (needle === null || product.name.toLowerCase().includes(needle)),
  );

  switch (sort) {
    case "name":
      items.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : a.id - b.id));
      break;
    case "price":
      items.sort((a, b) => a.price_cents - b.price_cents || a.id - b.id);
      break;
    case "price-desc":
      items.sort((a, b) => b.price_cents - a.price_cents || a.id - b.id);
      break;
    case "rating":
      items.sort((a, b) => b.rating_tenths - a.rating_tenths || a.id - b.id);
      break;
  }

  const total = items.length;
  const pageCount = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const current = Math.min(Math.max(page, 1), pageCount);
  const start = (current - 1) * PAGE_SIZE;

  return { items: items.slice(start, start + PAGE_SIZE), current, pageCount, total };
}

export function formatPrice(cents: number): string {
  return `$${Math.floor(cents / 100)}.${String(cents % 100).padStart(2, "0")}`;
}

export function formatRating(tenths: number): string {
  return `${Math.floor(tenths / 10)}.${tenths % 10}`;
}

export function filledStars(tenths: number): number {
  return Math.floor((tenths + 5) / 10);
}

export function initials(name: string): string {
  return name
    .split(/\s+/)
    .slice(0, 2)
    .map((word) => word[0] ?? "")
    .join("");
}
