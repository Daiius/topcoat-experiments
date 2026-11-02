/**
 * Longest search query the catalog accepts, in characters. Longer input is
 * cut off; the query comes from the browser and is untrusted.
 */
export const MAX_SEARCH_CHARS = 64;

/**
 * Builds a `/products` URL with the canonical query parameter order
 * (`page`, `sort`, `category`, `q`), omitting defaults. Mirrors
 * `topcoat/src/urls.rs` so rendered documents can be diffed.
 */
export function productsUrl(
  page: number,
  sort: string | null,
  category: string | null,
  search: string | null,
): string {
  let url = "/products";
  let sep = "?";

  if (page > 1) {
    url += `${sep}page=${page}`;
    sep = "&";
  }
  if (sort !== null) {
    url += `${sep}sort=${sort}`;
    sep = "&";
  }
  if (category !== null) {
    url += `${sep}category=${category}`;
    sep = "&";
  }
  if (search !== null) {
    url += `${sep}q=${encodeQueryValue(search)}`;
  }

  return url;
}

/** Percent-encodes everything except RFC 3986 unreserved characters. */
function encodeQueryValue(value: string): string {
  return encodeURIComponent(value).replace(
    /[!'()*]/g,
    (char) => `%${char.charCodeAt(0).toString(16).toUpperCase()}`,
  );
}

/** Normalizes the requested sort order. Unknown values select ascending ID order. */
export function normalizeSort(sort: string | undefined): string | null {
  switch (sort) {
    case "name":
    case "price":
    case "price-desc":
    case "rating":
      return sort;
    default:
      return null;
  }
}

/**
 * Normalizes a search query: trims it, caps its length (in code points, like
 * Rust's `chars()`), and treats an empty query as no search.
 */
export function normalizeSearch(search: string | undefined): string | null {
  if (search === undefined) {
    return null;
  }
  const trimmed = Array.from(search.trim()).slice(0, MAX_SEARCH_CHARS).join("").trimEnd();
  return trimmed === "" ? null : trimmed;
}

/** The first value of a search param that may repeat (`?q=a&q=b`). */
export function firstParam(value: string | string[] | undefined): string | undefined {
  return Array.isArray(value) ? value[0] : value;
}
