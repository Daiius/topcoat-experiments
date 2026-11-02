// The shopping cart, kept entirely in a cookie so any instance can serve any
// request. The server stores nothing.
//
// Next.js has no signed or encrypted cookie API (`cookies()` reads and writes
// plain values), so the cookie is unsigned. A visitor can edit it, but only
// their own cart: every read is validated against the catalog and clamped,
// and prices always come from the catalog, never from the cookie.
import { cookies } from "next/headers";

import { COUNT_COOKIE } from "./cart-count";
import { type Product, isProductId, lookupProducts } from "./catalog";

/** Most units of one product a cart line can hold. */
export const MAX_QUANTITY = 99;

/**
 * Most distinct products a cart can hold. Keeps the cookie far below the
 * browser's 4 KB limit (each line is about 12 bytes of JSON).
 */
export const MAX_LINES = 50;

const COOKIE = "cart";

/** `[product ID, quantity]` pairs in the order they were added: `[[12,1],[7,3]]`. */
export type Cart = [number, number][];

/** Parses a cookie value. Anything malformed reads as an empty cart. */
function parse(value: string | undefined): Cart {
  if (value === undefined) {
    return [];
  }
  let raw: unknown;
  try {
    raw = JSON.parse(value);
  } catch {
    return [];
  }
  if (!Array.isArray(raw)) {
    return [];
  }
  const cart: Cart = [];
  for (const line of raw) {
    if (!Array.isArray(line) || line.length !== 2) {
      continue;
    }
    const [id, quantity] = line;
    if (
      isProductId(id) &&
      Number.isInteger(quantity) &&
      quantity > 0 &&
      !cart.some(([existing]) => existing === id)
    ) {
      cart.push([id, Math.min(quantity, MAX_QUANTITY)]);
    }
  }
  return cart.slice(0, MAX_LINES);
}

/** The visitor's cart. Reads the request cookie, so callers are dynamic. */
export async function readCart(): Promise<Cart> {
  return parse((await cookies()).get(COOKIE)?.value);
}

/**
 * Replaces the cart cookie, and the readable count the header shows (so no
 * page has to read the cart on the server just for the header). Only
 * callable from a Server Action or Route Handler.
 */
export async function writeCart(cart: Cart): Promise<void> {
  const jar = await cookies();
  const options = { path: "/", sameSite: "lax", secure: true } as const;
  jar.set(COOKIE, JSON.stringify(cart), { ...options, httpOnly: true });
  jar.set(COUNT_COOKIE, String(countItems(cart)), { ...options, httpOnly: false });
}

/** Total units across all lines, as shown in the header. */
export function countItems(cart: Cart): number {
  return cart.reduce((sum, [, quantity]) => sum + quantity, 0);
}

/**
 * Adds `quantity` units of a product, capped at `MAX_QUANTITY`. A new
 * product is ignored once the cart holds `MAX_LINES` products.
 */
export function addLine(cart: Cart, productId: number, quantity: number): Cart {
  if (cart.some(([id]) => id === productId)) {
    return cart.map(([id, current]) =>
      id === productId ? [id, Math.min(current + quantity, MAX_QUANTITY)] : [id, current],
    );
  }
  return cart.length < MAX_LINES
    ? [...cart, [productId, Math.min(quantity, MAX_QUANTITY)]]
    : cart;
}

/** Sets a line's quantity, capped at `MAX_QUANTITY`. Zero removes it. */
export function setLine(cart: Cart, productId: number, quantity: number): Cart {
  if (quantity === 0) {
    return cart.filter(([id]) => id !== productId);
  }
  return cart.map(([id, current]) =>
    id === productId ? [id, Math.min(quantity, MAX_QUANTITY)] : [id, current],
  );
}

/** Lines joined with their products. */
export function cartItems(cart: Cart): [Product, number][] {
  const products = new Map(lookupProducts(cart.map(([id]) => id)).map((p) => [p.id, p]));
  return cart.flatMap(([id, quantity]): [Product, number][] => {
    const product = products.get(id);
    return product ? [[product, quantity]] : [];
  });
}

/** Total price of `items`, in cents. */
export function totalCents(items: [Product, number][]): number {
  return items.reduce((sum, [product, quantity]) => sum + product.price_cents * quantity, 0);
}
