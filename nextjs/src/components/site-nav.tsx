import Link from "next/link";
import { Suspense } from "react";

import { countItems, readCart } from "@/lib/cart";

export function SiteNav() {
  return (
    <header className="border-b border-slate-200 bg-white">
      <nav className="mx-auto flex w-full max-w-6xl items-center justify-between px-4 py-4">
        <Link href="/" className="text-lg font-bold tracking-tight">
          Meridian Supply
        </Link>
        <div className="flex items-center gap-6 text-sm font-medium text-slate-600">
          <Link href="/" className="hover:text-slate-900">
            Home
          </Link>
          <Link href="/products" className="hover:text-slate-900">
            Products
          </Link>
          {/* The only per-visitor part of the layout. The fallback ships in
              every page's static shell; the count streams in. */}
          <Suspense fallback={<CartLink label="Cart" />}>
            <CartCount />
          </Suspense>
        </div>
      </nav>
    </header>
  );
}

async function CartCount() {
  const count = countItems(await readCart());
  return <CartLink label={`Cart (${count})`} />;
}

function CartLink({ label }: { label: string }) {
  return (
    <Link
      href="/cart"
      className="rounded-full bg-indigo-600 px-3 py-1 text-xs font-semibold text-white hover:bg-indigo-500"
    >
      {label}
    </Link>
  );
}
