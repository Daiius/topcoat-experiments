mod _components;
mod cart;
mod products;

use topcoat::{
    Result,
    asset::{Asset, AssetBundle, AssetConfig, RouterBuilderAssetExt, asset},
    context::{Cx, app_context},
    cookie::{Key, RouterBuilderCookieExt},
    router::{Router, RouterBuilderDiscoverExt, Slot, layout, page},
    runtime::RouterBuilderRuntimeExt,
    view::{View, view},
};

use crate::{
    app::_components::{product_card, site_footer, site_nav},
    cart::signing_key,
    catalog::Catalog,
};

const TAILWIND_CSS: Asset = asset!("assets/tailwind.css");

pub fn router() -> Router {
    build(
        AssetBundle::load()
            .expect("asset bundle loads; run `topcoat asset bundle`")
            .into(),
        signing_key(),
    )
}

fn build(assets: AssetConfig, cookie_key: Key) -> Router {
    topcoat::router::module_router!()
        .app_context(Catalog::load())
        // Signs the cart cookie. Load the same key on every instance.
        .cookies()
        .app_context(cookie_key)
        .assets(assets)
        // Registers the shards (and procedures) declared anywhere in the app.
        .discover()
        // Layers that should see runtime reruns as plain GETs go above this.
        .runtime()
        .build()
}

#[layout]
async fn root_layout(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Meridian Supply"</title>
                <link rel="stylesheet" href=(TAILWIND_CSS)>
                // The browser runtime for shards and live regions. It loads on
                // every page because the layout is shared.
                topcoat::runtime::script()
            </head>
            <body class="flex min-h-screen flex-col bg-slate-50 text-slate-900">
                site_nav()
                <main class="mx-auto w-full max-w-6xl flex-1 px-4 py-8">(slot)</main>
                site_footer()
            </body>
        </html>
    })
}

#[page]
async fn home(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <section class="rounded-2xl bg-indigo-600 px-8 py-16 text-white">
            <h1 class="max-w-2xl text-4xl font-bold tracking-tight">
                "Gear that earns its place on your desk"
            </h1>
            <p class="mt-4 max-w-xl text-lg text-indigo-100">
                "Five hundred products, zero filler. Everything in the catalog is tested daily by the people who build it."
            </p>
            <a
                href="/products"
                class="mt-8 inline-block rounded-lg bg-white px-6 py-3 text-sm font-semibold text-indigo-700 hover:bg-indigo-50"
            >
                "Browse all products"
            </a>
        </section>
        <section class="mt-12">
            <div class="flex items-baseline justify-between">
                <h2 class="text-2xl font-bold tracking-tight">"Featured products"</h2>
                <a
                    href="/products"
                    class="text-sm font-medium text-indigo-600 hover:text-indigo-500"
                >
                    "View all"
                </a>
            </div>
            <div class="mt-6 grid grid-cols-2 gap-6 md:grid-cols-3 lg:grid-cols-4">
                for product in app_context::<Catalog>(cx).featured() {
                    product_card(product: product)
                }
            </div>
        </section>
    })
}

/// End-to-end tests through the real router: module pages, layout, cookies,
/// and the runtime, with placeholder asset URLs instead of a bundle.
#[cfg(test)]
mod tests {
    use topcoat::router::{
        Body, StatusCode,
        header::{CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE},
        request::Request,
        response::Response,
        to_bytes,
    };

    use super::*;
    use crate::images::test_asset_config;

    fn test_router() -> Router {
        build(
            test_asset_config(&[
                (TAILWIND_CSS, "tailwind.css"),
                (topcoat::runtime::SCRIPT, "topcoat.js"),
            ]),
            Key::generate(),
        )
    }

    async fn send(router: &Router, request: Request) -> (Response, String) {
        let response = router.handle(request).await;
        let (parts, body) = response.into_parts();
        let bytes = to_bytes(body, usize::MAX).await.unwrap();
        (
            Response::from_parts(parts, Body::empty()),
            String::from_utf8(bytes.to_vec()).unwrap(),
        )
    }

    async fn get(router: &Router, path: &str, cookie: Option<&str>) -> String {
        let mut request = Request::builder().uri(path);
        if let Some(cookie) = cookie {
            request = request.header(COOKIE, cookie);
        }
        let (response, html) = send(router, request.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::OK, "{html}");
        html
    }

    /// Posts a form and returns the response and the `name=value` part of
    /// the cart cookie it sets, if any.
    async fn post(
        router: &Router,
        path: &str,
        form: &str,
        cookie: Option<&str>,
    ) -> (Response, Option<String>) {
        let mut request = Request::builder()
            .method("POST")
            .uri(path)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded");
        if let Some(cookie) = cookie {
            request = request.header(COOKIE, cookie);
        }
        let (response, _) = send(router, request.body(Body::from(form.to_owned())).unwrap()).await;
        let cart_cookie = response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap())
            .find(|value| value.starts_with("cart="))
            .map(|value| value.split(';').next().unwrap().to_owned());
        (response, cart_cookie)
    }

    #[tokio::test]
    async fn an_empty_cart_shows_zero_and_a_hint() {
        let router = test_router();
        let html = get(&router, "/cart", None).await;
        assert!(html.contains("Cart (0)"), "{html}");
        assert!(html.contains("Your cart is empty."), "{html}");
    }

    #[tokio::test]
    async fn adding_updating_and_removing_round_trip_through_the_cookie() {
        let router = test_router();
        let first = Catalog::load().get(1).unwrap().name.clone();
        let second = Catalog::load().get(2).unwrap().name.clone();

        let (response, cookie) = post(&router, "/cart/add", "product_id=1&quantity=2", None).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[LOCATION], "/cart");
        let cookie = cookie.expect("adding sets the cart cookie");
        let set_cookie = response.headers()[SET_COOKIE].to_str().unwrap();
        assert!(set_cookie.contains("HttpOnly"), "{set_cookie}");
        assert!(set_cookie.contains("SameSite=Lax"), "{set_cookie}");

        let (_, cookie) = post(&router, "/cart/add", "product_id=2", Some(&cookie)).await;
        let cookie = cookie.unwrap();
        let html = get(&router, "/cart", Some(&cookie)).await;
        assert!(html.contains("Cart (3)"), "{html}");
        assert!(html.contains(&first) && html.contains(&second), "{html}");

        let (_, cookie) = post(
            &router,
            "/cart/update",
            "product_id=1&quantity=5",
            Some(&cookie),
        )
        .await;
        let cookie = cookie.unwrap();
        assert!(
            get(&router, "/cart", Some(&cookie))
                .await
                .contains("Cart (6)")
        );

        let (_, cookie) = post(
            &router,
            "/cart/update",
            "product_id=1&quantity=0",
            Some(&cookie),
        )
        .await;
        let cookie = cookie.unwrap();
        let html = get(&router, "/cart", Some(&cookie)).await;
        assert!(html.contains("Cart (1)"), "{html}");
        assert!(!html.contains(&first), "{html}");
    }

    #[tokio::test]
    async fn a_tampered_cookie_reads_as_an_empty_cart() {
        let router = test_router();
        let (_, cookie) = post(&router, "/cart/add", "product_id=1", None).await;
        // Swap the signed value's payload for one claiming 99 units.
        let cookie = cookie.unwrap();
        let (name_and_signature, _) = cookie.split_at(cookie.find("%5B").unwrap());
        let forged = format!("{name_and_signature}%5B%5B1%2C99%5D%5D");
        let html = get(&router, "/cart", Some(&forged)).await;
        assert!(html.contains("Cart (0)"), "{html}");
    }

    #[tokio::test]
    async fn unknown_products_cannot_be_added() {
        let router = test_router();
        let (response, cookie) = post(&router, "/cart/add", "product_id=9999", None).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(cookie.is_none());
    }

    #[tokio::test]
    async fn the_products_page_renders_the_search_shard_and_runtime() {
        let router = test_router();
        let html = get(&router, "/products?q=headphones&sort=price", None).await;
        assert!(html.contains("14 products matching"), "{html}");
        assert!(html.contains("::topcoat::shard::start"), "{html}");
        assert!(html.contains("https://assets.test/topcoat.js"), "{html}");
    }
}
