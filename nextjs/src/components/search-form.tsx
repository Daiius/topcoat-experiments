import Form from "next/form";

import { MAX_SEARCH_CHARS } from "@/lib/urls";

/**
 * The product search box. A GET form: without JavaScript it is a plain
 * navigation to `/products?…&q=…`; with it, `next/form` turns the submit into
 * a client-side navigation and prefetches the route when visible. Results
 * update on Enter, not per keystroke.
 */
export function SearchForm({
  sort,
  category,
  search,
}: {
  sort: string | null;
  category: string | null;
  search: string | null;
}) {
  return (
    <Form action="/products" role="search" className="w-full sm:w-72">
      {sort !== null && <input type="hidden" name="sort" value={sort} />}
      {category !== null && <input type="hidden" name="category" value={category} />}
      <input
        // Navigations keep this component mounted; a new query resets the box.
        key={search ?? ""}
        type="search"
        name="q"
        placeholder="Search products"
        aria-label="Search products"
        autoComplete="off"
        maxLength={MAX_SEARCH_CHARS}
        defaultValue={search ?? ""}
        className="w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm shadow-sm focus:border-indigo-500 focus:outline-none"
      />
    </Form>
  );
}
