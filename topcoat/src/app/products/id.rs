use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::RouterErrorExt, href, module_param, page, path_param},
    runtime::{connected, shard},
    view::{View, component, emit, live, suspense, view},
};

use crate::{
    app::_components::{
        breadcrumbs, product_card, product_card_skeleton, product_photo, rating_stars, review_list,
        review_list_skeleton, spec_table,
    },
    cart::MAX_QUANTITY,
    catalog::{Catalog, format_rating},
    remote::{fetch_related, fetch_reviews},
    stock::{LOW_STOCK, stock_at, until_next_change},
};

module_param!(product_id: u32, error = not_found);

#[page]
async fn product_detail(cx: &Cx) -> Result<impl View> {
    let catalog = app_context::<Catalog>(cx);
    let product = catalog
        .get(*path_param::<ProductId>(cx)?)
        .ok_or_not_found()?;

    Ok(view! {
        breadcrumbs(
            category: &product.category,
            category_slug: &product.category_slug,
            name: &product.name
        )
        <div class="mt-6 grid gap-10 lg:grid-cols-2">
            // The likely LCP element: load it eagerly.
            product_photo(
                product: product,
                frame: "aspect-square rounded-2xl",
                eager: true
            )
            <div>
                <p class="text-sm font-medium uppercase tracking-wide text-slate-400">
                    (&product.category)
                </p>
                <h1 class="mt-1 text-3xl font-bold tracking-tight">(&product.name)</h1>
                <div class="mt-3 flex items-center gap-2">
                    rating_stars(tenths: product.rating_tenths, size: "h-5 w-5")
                    <span class="text-sm text-slate-500">
                        (format_rating(product.rating_tenths))
                        " ("
                        (product.review_count)
                        " reviews)"
                    </span>
                </div>
                <p class="mt-6 text-4xl font-bold">(product.price())</p>
                stock_level(product_id: product.id)
                <div class="mt-6 space-y-4 text-slate-600">
                    for paragraph in &product.description {
                        <p>(paragraph)</p>
                    }
                </div>
                // A plain form POST: the handler updates the signed cart
                // cookie and redirects to the cart page.
                <form
                    method="post"
                    action=(href!(crate::app::cart::add::add))
                    class="mt-8 flex items-center gap-3"
                >
                    <input type="hidden" name="product_id" value=(product.id)>
                    <label class="sr-only" for="quantity">"Quantity"</label>
                    <input
                        id="quantity"
                        type="number"
                        name="quantity"
                        min="1"
                        max=(MAX_QUANTITY)
                        value="1"
                        class="w-20 rounded-lg border border-slate-300 px-3 py-3 text-sm"
                    >
                    <button
                        type="submit"
                        class="inline-block rounded-lg bg-indigo-600 px-8 py-3 text-sm font-semibold text-white hover:bg-indigo-500"
                    >
                        "Add to cart"
                    </button>
                </form>
            </div>
        </div>
        <section class="mt-16">
            <h2 class="text-2xl font-bold tracking-tight">"Specifications"</h2>
            spec_table(specs: &product.specs)
        </section>
        <section class="mt-16">
            <h2 class="text-2xl font-bold tracking-tight">
                "Reviews ("
                (product.review_count)
                ")"
            </h2>
            suspense(
                fallback: view! { review_list_skeleton(count: product.review_count.min(3)) },
                reviews(product_id: product.id)
            )
        </section>
        <section class="mt-16">
            <h2 class="text-2xl font-bold tracking-tight">"Related products"</h2>
            suspense(
                fallback: view! { related_grid_skeleton(count: product.related_ids.len()) },
                related(product_id: product.id)
            )
        </section>
    })
}

/// The live stock level. The page's HTTP response carries the current level
/// and finishes; `connected` then has the browser open a WebSocket and run
/// this shard again, and that run keeps emitting a fresh level every
/// interval. Only the shard reconnects, not the whole page.
#[shard("/_shards/stock-level")]
async fn stock_level(cx: &Cx, product_id: u32) -> Result<impl View> {
    // The ID comes back from the browser on reconnect: check it.
    let known = app_context::<Catalog>(cx).get(product_id).is_some();
    Ok(live! {
        loop {
            let now = std::time::SystemTime::now();
            let level = stock_at(product_id, now);
            let token = emit! {
                if !known {

                } else if level == 0 {
                    <p class="mt-3 text-sm font-semibold text-rose-600">
                        "Out of stock"
                    </p>
                } else if level <= LOW_STOCK {
                    <p class="mt-3 text-sm font-semibold text-amber-600">
                        "Only "
                        (level)
                        " left in stock"
                    </p>
                } else {
                    <p class="mt-3 text-sm font-medium text-emerald-700">
                        (level)
                        " in stock"
                    </p>
                }
            }?;
            if !known || !connected(cx) {
                break Ok(token);
            }
            tokio::time::sleep(until_next_change(now)).await;
        }
    })
}

/// Reviews come from a slow service, so they stream in after the shell.
#[component]
async fn reviews(cx: &Cx, product_id: u32) -> Result<impl View> {
    let reviews = fetch_reviews(app_context::<Catalog>(cx), product_id).await;
    Ok(view! { review_list(reviews: reviews) })
}

/// Recommendations come from an even slower service and stream in last.
#[component]
async fn related(cx: &Cx, product_id: u32) -> Result<impl View> {
    let related = fetch_related(app_context::<Catalog>(cx), product_id).await;
    Ok(view! {
        <div class="mt-6 grid grid-cols-2 gap-6 md:grid-cols-4">
            for product in &related {
                product_card(product: product)
            }
        </div>
    })
}

#[component]
async fn related_grid_skeleton(count: usize) -> Result<impl View> {
    Ok(view! {
        <div class="mt-6 grid grid-cols-2 gap-6 md:grid-cols-4">
            for _ in 0..count {
                product_card_skeleton()
            }
        </div>
    })
}

#[cfg(test)]
mod tests {
    use tokio::time::Instant;
    use topcoat::{
        asset::RouterBuilderAssetExt,
        router::{Body, Router, request::Request, to_bytes},
    };

    use super::*;
    use crate::{images::test_asset_config, remote::RELATED_DELAY};

    // Module-derived pages cannot be registered by hand, and the real layout
    // needs the asset bundle, so this page mounts the same two boundaries
    // `product_detail` uses on an explicit path.
    #[page("/boundaries")]
    async fn boundaries() -> Result<impl View> {
        Ok(view! {
            suspense(
                fallback: view! { review_list_skeleton(count: 1) },
                reviews(product_id: 1)
            )
            suspense(
                fallback: view! { related_grid_skeleton(count: 1) },
                related(product_id: 1)
            )
        })
    }

    #[tokio::test(start_paused = true)]
    async fn sections_stream_in_after_their_fallbacks() {
        let router = Router::builder()
            .app_context(Catalog::load())
            .assets(test_asset_config(&[]))
            .page(boundaries)
            .build();
        let request = Request::builder()
            .uri("/boundaries")
            .body(Body::empty())
            .unwrap();

        let started = Instant::now();
        let response = router.handle(request).await;
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8(bytes.to_vec()).unwrap();

        // Both services are awaited concurrently, not one after the other.
        assert_eq!(started.elapsed(), RELATED_DELAY);

        let position = |needle: &str| {
            html.find(needle)
                .unwrap_or_else(|| panic!("{needle:?} missing from {html}"))
        };
        let catalog = Catalog::load();
        let product = catalog.get(1).unwrap();
        let first_review = &product.reviews[0].title;
        let first_related = &catalog.get(product.related_ids[0]).unwrap().name;

        // The fallbacks go out first, then each section arrives as a swap,
        // the faster reviews before the slower recommendations.
        let reviews_fallback = position("Loading reviews");
        let first_swap = position("<template data-topcoat-swap");
        let review_at = position(first_review);
        let related_at = position(first_related);
        assert!(reviews_fallback < first_swap, "{html}");
        assert!(first_swap < review_at, "{html}");
        assert!(review_at < related_at, "{html}");
    }
}
