// The header's cart count, shared by the server (which writes it) and the
// browser (which reads it). No `next/headers` here, so a Client Component
// can import it; `lib/cart.ts` cannot be.

/**
 * Display-only copy of the cart's item count, readable from JavaScript. The
 * cart itself stays in the HttpOnly `cart` cookie; `writeCart` sets both.
 */
export const COUNT_COOKIE = "cart_count";

/** Highest count a cart can reach (50 lines × 99 units). */
const MAX_COUNT = 50 * 99;

/** The count in a `document.cookie` string; 0 when absent or malformed. */
export function parseCount(cookieString: string): number {
  const prefix = `${COUNT_COOKIE}=`;
  const entry = cookieString.split("; ").find((part) => part.startsWith(prefix));
  const value = Number(entry?.slice(prefix.length));
  return Number.isInteger(value) && value >= 0 ? Math.min(value, MAX_COUNT) : 0;
}
