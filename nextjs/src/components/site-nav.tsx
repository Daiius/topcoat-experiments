import Link from "next/link";

import { CartCount } from "./cart-count";

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
          {/* The only per-visitor part of the layout, read in the browser so
              the server never touches the request here. */}
          <CartCount />
        </div>
      </nav>
    </header>
  );
}
