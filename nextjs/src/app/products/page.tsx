import Link from "next/link";
import { Suspense } from "react";

import { Pagination } from "@/components/pagination";
import { ProductCard } from "@/components/product-card";
import { SearchForm } from "@/components/search-form";
import { ProductGridSkeleton } from "@/components/skeletons";
import { getCategories, getProductsPage } from "@/lib/catalog";
import { firstParam, normalizeSearch, normalizeSort, productsUrl } from "@/lib/urls";

const SORT_OPTIONS: [string | null, string][] = [
  [null, "Default"],
  ["name", "Name"],
  ["price", "Price: Low to high"],
  ["price-desc", "Price: High to low"],
  ["rating", "Rating"],
];

const CHIP_ACTIVE = "rounded-full bg-slate-900 px-3 py-1 font-medium text-white";
const CHIP_INACTIVE =
  "rounded-full bg-white px-3 py-1 font-medium text-slate-600 shadow-sm hover:bg-slate-100";

const GRID = "mt-4 grid grid-cols-2 gap-6 md:grid-cols-3 lg:grid-cols-4";

type SearchParams = PageProps<"/products">["searchParams"];

interface ProductsQuery {
  page: number;
  sort: string | null;
  category: string | null;
  search: string | null;
}

/** Parses the query string. Every value is user input. */
async function readQuery(searchParams: SearchParams): Promise<ProductsQuery> {
  const params = await searchParams;
  const requested = Number.parseInt(firstParam(params.page) ?? "", 10);
  return {
    page: Number.isNaN(requested) ? 1 : requested,
    sort: normalizeSort(firstParam(params.sort)),
    category: firstParam(params.category) ?? null,
    search: normalizeSearch(firstParam(params.q)),
  };
}

// Not async: the page never awaits `searchParams` itself. The heading, an
// empty search box, and the grid skeleton form the static shell; the parts
// that depend on the URL stream in.
export default function ProductsPage({ searchParams }: PageProps<"/products">) {
  return (
    <>
      <div className="flex flex-wrap items-center justify-between gap-4">
        <h1 className="text-3xl font-bold tracking-tight">All products</h1>
        <Suspense fallback={<SearchForm sort={null} category={null} search={null} />}>
          <SearchFormForQuery searchParams={searchParams} />
        </Suspense>
      </div>
      <Suspense fallback={<ProductGridSkeleton count={8} className={`mt-24 ${GRID}`} />}>
        <ProductResults searchParams={searchParams} />
      </Suspense>
    </>
  );
}

/** The search box, prefilled and carrying the current sort and category. */
async function SearchFormForQuery({ searchParams }: { searchParams: SearchParams }) {
  const { sort, category, search } = await readQuery(searchParams);
  return <SearchForm sort={sort} category={category} search={search} />;
}

/** The filter chips, result count, grid, and pagination for the URL. */
async function ProductResults({ searchParams }: { searchParams: SearchParams }) {
  const { page: requested, sort, category, search } = await readQuery(searchParams);
  // Cached per argument tuple: the filtering runs once per distinct query.
  const page = await getProductsPage(requested, sort, category, search);

  return (
    <>
      <div className="mt-6 flex flex-wrap items-center gap-2 text-sm">
        <span className="font-medium text-slate-500">Sort:</span>
        {SORT_OPTIONS.map(([value, label]) => (
          <Link
            key={label}
            href={productsUrl(1, value, category, search)}
            className={value === sort ? CHIP_ACTIVE : CHIP_INACTIVE}
          >
            {label}
          </Link>
        ))}
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-2 text-sm">
        <span className="font-medium text-slate-500">Category:</span>
        <Link
          href={productsUrl(1, sort, null, search)}
          className={category === null ? CHIP_ACTIVE : CHIP_INACTIVE}
        >
          All
        </Link>
        {getCategories().map((entry) => (
          <Link
            key={entry.slug}
            href={productsUrl(1, sort, entry.slug, search)}
            className={category === entry.slug ? CHIP_ACTIVE : CHIP_INACTIVE}
          >
            {entry.name}
          </Link>
        ))}
      </div>
      <p className="mt-6 text-sm text-slate-500" aria-live="polite">
        {search === null
          ? `${page.total} products`
          : `${page.total} products matching “${search}”`}
      </p>
      {page.total === 0 ? (
        <p className="mt-8 text-slate-500">No products found.</p>
      ) : (
        <>
          <div className={GRID}>
            {page.items.map((product) => (
              <ProductCard key={product.id} product={product} />
            ))}
          </div>
          <Pagination
            current={page.current}
            pageCount={page.pageCount}
            sort={sort}
            category={category}
            search={search}
          />
        </>
      )}
    </>
  );
}
