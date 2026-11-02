import Link from "next/link";

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
          <span className="rounded-full bg-indigo-600 px-3 py-1 text-xs font-semibold text-white">
            Cart (0)
          </span>
        </div>
      </nav>
    </header>
  );
}
