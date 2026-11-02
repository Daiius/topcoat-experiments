import Link from "next/link";

export function Breadcrumbs({
  category,
  categorySlug,
  name,
}: {
  category: string;
  categorySlug: string;
  name: string;
}) {
  return (
    <nav aria-label="Breadcrumb" className="text-sm text-slate-500">
      <ol className="flex flex-wrap items-center gap-2">
        <li>
          <Link href="/" className="hover:text-slate-900">
            Home
          </Link>
        </li>
        <li>/</li>
        <li>
          <Link href="/products" className="hover:text-slate-900">
            Products
          </Link>
        </li>
        <li>/</li>
        <li>
          <Link href={`/products?category=${categorySlug}`} className="hover:text-slate-900">
            {category}
          </Link>
        </li>
        <li>/</li>
        <li className="font-medium text-slate-900">{name}</li>
      </ol>
    </nav>
  );
}
