mod id;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{page, query_params},
    runtime::{Event, shard, signal},
    view::{StaticClass, View, class, view},
};

use crate::{
    app::_components::{pagination, product_card},
    catalog::Catalog,
    urls::{MAX_SEARCH_CHARS, normalize_search, normalize_sort, products_url},
};

const SORT_OPTIONS: [(Option<&str>, &str); 5] = [
    (None, "Default"),
    (Some("name"), "Name"),
    (Some("price"), "Price: Low to high"),
    (Some("price-desc"), "Price: High to low"),
    (Some("rating"), "Rating"),
];

const CHIP_ACTIVE: StaticClass =
    class!("rounded-full bg-slate-900 px-3 py-1 font-medium text-white");
const CHIP_INACTIVE: StaticClass = class!(
    "rounded-full bg-white px-3 py-1 font-medium text-slate-600 shadow-sm hover:bg-slate-100",
);

#[query_params(error = bad_request)]
struct ProductsQuery {
    page: Option<usize>,
    sort: Option<String>,
    category: Option<String>,
    q: Option<String>,
}

#[page]
async fn products(cx: &Cx) -> Result<impl View> {
    let query = query_params::<ProductsQuery>(cx)?;
    let sort = normalize_sort(query.sort.as_deref()).map(str::to_owned);
    let category = query.category.clone();
    let initial_search = query
        .q
        .as_deref()
        .and_then(normalize_search)
        .unwrap_or_default();
    let initial_page = query.page.unwrap_or(1);

    // The search box lives in the browser. Typing re-renders only the
    // `product_results` shard; the rest of the page stays as it is.
    let search = signal(cx, || initial_search.clone());

    Ok(view! {
        <div class="flex flex-wrap items-center justify-between gap-4">
            <h1 class="text-3xl font-bold tracking-tight">"All products"</h1>
            // Without JavaScript, Enter submits this form as a plain GET.
            <form method="get" action="/products" role="search" class="w-full sm:w-72">
                if let Some(sort) = &sort {
                    <input type="hidden" name="sort" value=(sort)>
                }
                if let Some(category) = &category {
                    <input type="hidden" name="category" value=(category)>
                }
                <input
                    type="search"
                    name="q"
                    placeholder="Search products"
                    aria-label="Search products"
                    autocomplete="off"
                    maxlength=(MAX_SEARCH_CHARS)
                    class="w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm shadow-sm focus:border-indigo-500 focus:outline-none"
                    :value=$(search.get())
                    @input=$(|e: Event| {
                        let value = e.target.value;
                        search.set(value.to_owned());
                        // Keep the address bar in step so reloads and shared
                        // links show the same results. Drops `page`, since a
                        // new query starts at the first page. The JavaScript
                        // must be an expression: the runtime lowers it to
                        // `return <js>;`. `${value}` is a runtime surrogate,
                        // not a JS string; `'' + …` converts it.
                        raw!(
                            "((url, q) => { if (q) url.searchParams.set('q', q); else url.searchParams.delete('q'); url.searchParams.delete('page'); history.replaceState(history.state, '', url); })(new URL(location.href), ('' + ${value}).trim())",
                            (),
                        )
                    })
                >
            </form>
        </div>
        product_results(
            search: $(search.get()),
            // Stay on the requested page until the query changes.
            page: $(if search.get() == initial_search { initial_page } else { 1 }),
            sort: sort.clone(),
            category: category.clone()
        )
    })
}

/// The filter chips, result grid, and pagination. Re-renders on the server
/// whenever its arguments change in the browser. Every argument is user
/// input, so each one is normalized again here.
#[shard("/_shards/product-results")]
async fn product_results(
    cx: &Cx,
    search: String,
    page: usize,
    sort: Option<String>,
    category: Option<String>,
) -> Result<impl View> {
    let catalog = app_context::<Catalog>(cx);
    let search = normalize_search(&search);
    let sort = normalize_sort(sort.as_deref());
    let page = catalog.page(page, sort, category.as_deref(), search.as_deref());
    // The view takes ownership of what it captures (like `async move`), so
    // it borrows from these owned values inside rather than from locals here.

    Ok(view! {
        <div class="mt-6 flex flex-wrap items-center gap-2 text-sm">
            <span class="font-medium text-slate-500">"Sort:"</span>
            for (value, label) in SORT_OPTIONS {
                <a
                    href=(products_url(1, value, category.as_deref(), search.as_deref()))
                    class=(class!(CHIP_ACTIVE if value == sort else CHIP_INACTIVE))
                >
                    (label)
                </a>
            }
        </div>
        <div class="mt-3 flex flex-wrap items-center gap-2 text-sm">
            <span class="font-medium text-slate-500">"Category:"</span>
            <a
                href=(products_url(1, sort, None, search.as_deref()))
                class=(class!(CHIP_ACTIVE if category.is_none() else CHIP_INACTIVE))
            >
                "All"
            </a>
            for entry in catalog.categories() {
                <a
                    href=(products_url(1, sort, Some(&entry.slug), search.as_deref()))
                    class=(class!(
                        CHIP_ACTIVE if category.as_deref() == Some(entry.slug.as_str()) else CHIP_INACTIVE,
                    ))
                >
                    (&entry.name)
                </a>
            }
        </div>
        <p class="mt-6 text-sm text-slate-500" aria-live="polite">
            (page.total)
            " products"
            if let Some(search) = &search {
                " matching \u{201c}"
                (search)
                "\u{201d}"
            }
        </p>
        if page.total == 0 {
            <p class="mt-8 text-slate-500">"No products found."</p>
        } else {
            <div class="mt-4 grid grid-cols-2 gap-6 md:grid-cols-3 lg:grid-cols-4">
                // Only the first card is fetched with high priority: it is
                // the likely LCP element, the rest stay lazy.
                for (index, product) in page.items.iter().enumerate() {
                    product_card(product: product, priority: index == 0)
                }
            </div>
            pagination(
                current: page.current,
                page_count: page.page_count,
                sort: sort,
                category: category.as_deref(),
                search: search.as_deref()
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use topcoat::{
        asset::RouterBuilderAssetExt,
        router::{Body, Router, request::Request, to_bytes},
    };

    use super::*;
    use crate::images::test_asset_config;

    // The shard's first render happens inside its page, like a component.
    // These pages call it with fixed arguments on explicit paths.
    #[page("/headphones")]
    async fn headphones() -> Result<impl View> {
        Ok(view! {
            product_results(
                search: "  HEADPHONES ".to_owned(),
                page: 1,
                sort: Some("price".to_owned()),
                category: None
            )
        })
    }

    #[page("/nothing")]
    async fn nothing() -> Result<impl View> {
        Ok(view! {
            product_results(
                search: "zzzz".to_owned(),
                page: 1,
                sort: None,
                category: None
            )
        })
    }

    async fn fetch(path: &str) -> String {
        let router = Router::builder()
            .app_context(Catalog::load())
            .assets(test_asset_config(&[]))
            .page(headphones)
            .page(nothing)
            .build();
        let request = Request::builder().uri(path).body(Body::empty()).unwrap();
        let response = router.handle(request).await;
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn search_filters_by_name_and_keeps_the_query_in_links() {
        let html = fetch("/headphones").await;
        assert!(html.contains("14 products matching"), "{html}");
        // Chips and pagination carry the normalized query.
        assert!(
            html.contains("/products?sort=name&amp;q=HEADPHONES"),
            "{html}"
        );
        assert!(!html.contains("No products found."), "{html}");
    }

    #[tokio::test]
    async fn an_unmatched_search_says_so() {
        let html = fetch("/nothing").await;
        assert!(html.contains("0 products matching"), "{html}");
        assert!(html.contains("No products found."), "{html}");
    }
}
