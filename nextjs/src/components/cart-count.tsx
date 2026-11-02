"use client";

import Link from "next/link";
import { useSyncExternalStore } from "react";

import { parseCount } from "@/lib/cart-count";

/** The count the last cart update wrote; 0 before the first one. */
function readCount(): number {
  return parseCount(document.cookie);
}

/**
 * Re-reads the count when a cookie changes (a Server Action's response sets
 * it). Without the Cookie Store API the count is re-read only when the
 * header happens to re-render, or on the next full page load.
 */
function subscribe(onChange: () => void): () => void {
  if (!("cookieStore" in window)) {
    return () => {};
  }
  window.cookieStore.addEventListener("change", onChange);
  return () => window.cookieStore.removeEventListener("change", onChange);
}

/** Prerender and hydration: no count, so the static HTML is the same for everyone. */
function serverCount(): null {
  return null;
}

/**
 * The header's cart link. The server never reads the cookie, so every page
 * that shows it can stay fully static. The prerendered HTML says "Cart" (as
 * the old Suspense fallback did); the count appears right after hydration.
 */
export function CartCount() {
  const count = useSyncExternalStore(subscribe, readCount, serverCount);
  return (
    <Link
      href="/cart"
      className="rounded-full bg-indigo-600 px-3 py-1 text-xs font-semibold text-white hover:bg-indigo-500"
    >
      {count === null ? "Cart" : `Cart (${count})`}
    </Link>
  );
}
