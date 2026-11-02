import Link from "next/link";

import { productsUrl } from "@/lib/urls";

const PAGE_LINK = "rounded-md px-3 py-2 font-medium text-slate-600 hover:bg-slate-100";
const PAGE_DISABLED = "rounded-md px-3 py-2 font-medium text-slate-300";
const PAGE_CURRENT = "rounded-md bg-indigo-600 px-3 py-2 font-semibold text-white";

export function Pagination({
  current,
  pageCount,
  sort,
  category,
  search,
}: {
  current: number;
  pageCount: number;
  sort: string | null;
  category: string | null;
  search: string | null;
}) {
  const numbers = Array.from({ length: pageCount }, (_, index) => index + 1);

  return (
    <nav
      aria-label="Pagination"
      className="mt-10 flex flex-wrap items-center justify-center gap-1 text-sm"
    >
      {current > 1 ? (
        <Link href={productsUrl(current - 1, sort, category, search)} className={PAGE_LINK}>
          Previous
        </Link>
      ) : (
        <span className={PAGE_DISABLED}>Previous</span>
      )}
      {numbers.map((number) =>
        number === current ? (
          <span key={number} aria-current="page" className={PAGE_CURRENT}>
            {number}
          </span>
        ) : (
          <Link
            key={number}
            href={productsUrl(number, sort, category, search)}
            className={PAGE_LINK}
          >
            {number}
          </Link>
        ),
      )}
      {current < pageCount ? (
        <Link href={productsUrl(current + 1, sort, category, search)} className={PAGE_LINK}>
          Next
        </Link>
      ) : (
        <span className={PAGE_DISABLED}>Next</span>
      )}
    </nav>
  );
}
