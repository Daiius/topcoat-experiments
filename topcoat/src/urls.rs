use std::fmt::Write;

/// Longest search query the catalog accepts, in characters. Longer input is
/// cut off; the query comes from the browser and is untrusted.
pub const MAX_SEARCH_CHARS: usize = 64;

/// Builds a `/products` URL with the canonical query parameter order
/// (`page`, `sort`, `category`, `q`), omitting defaults. The Next.js version
/// builds identical URLs so rendered documents can be diffed.
pub fn products_url(
    page: usize,
    sort: Option<&str>,
    category: Option<&str>,
    search: Option<&str>,
) -> String {
    let mut url = String::from("/products");
    let mut sep = '?';

    if page > 1 {
        url.push(sep);
        sep = '&';
        url.push_str("page=");
        url.push_str(&page.to_string());
    }
    if let Some(sort) = sort {
        url.push(sep);
        sep = '&';
        url.push_str("sort=");
        url.push_str(sort);
    }
    if let Some(category) = category {
        url.push(sep);
        sep = '&';
        url.push_str("category=");
        url.push_str(category);
    }
    if let Some(search) = search {
        url.push(sep);
        url.push_str("q=");
        encode_query_value(search, &mut url);
    }

    url
}

/// Percent-encodes everything except RFC 3986 unreserved characters, the
/// same output as JavaScript's `encodeURIComponent` minus `!'()*`.
fn encode_query_value(value: &str, out: &mut String) {
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
}

/// Normalizes the requested sort order. Unknown values select ascending ID order.
pub fn normalize_sort(sort: Option<&str>) -> Option<&'static str> {
    match sort {
        Some("name") => Some("name"),
        Some("price") => Some("price"),
        Some("price-desc") => Some("price-desc"),
        Some("rating") => Some("rating"),
        _ => None,
    }
}

/// Normalizes a search query: trims it, caps its length, and treats an empty
/// query as no search.
pub fn normalize_search(search: &str) -> Option<String> {
    let trimmed: String = search.trim().chars().take(MAX_SEARCH_CHARS).collect();
    let trimmed = trimmed.trim_end();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_keep_the_canonical_order_and_omit_defaults() {
        assert_eq!(products_url(1, None, None, None), "/products");
        assert_eq!(
            products_url(2, Some("price"), Some("audio"), Some("pro max")),
            "/products?page=2&sort=price&category=audio&q=pro%20max"
        );
        assert_eq!(
            products_url(1, None, None, Some("a&b=c")),
            "/products?q=a%26b%3Dc"
        );
    }

    #[test]
    fn search_is_trimmed_capped_and_optional() {
        assert_eq!(normalize_search("   "), None);
        assert_eq!(
            normalize_search("  Desk Lamp "),
            Some("Desk Lamp".to_owned())
        );
        let long = "x".repeat(MAX_SEARCH_CHARS + 10);
        assert_eq!(normalize_search(&long).unwrap().len(), MAX_SEARCH_CHARS);
    }
}
