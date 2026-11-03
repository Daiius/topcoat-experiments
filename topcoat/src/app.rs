mod _components;
mod cart;
mod products;

use topcoat::{
    Result,
    asset::{Asset, AssetBundle, AssetConfig, RouterBuilderAssetExt, asset},
    context::{Cx, app_context},
    cookie::{Key, RouterBuilderCookieExt},
    router::{
        HeaderValue, Method, Router, RouterBuilderDiscoverExt, Slot, header, layout, page,
        request::{original_method, original_parts},
        response::response_headers,
    },
    runtime::{RUNTIME_HEADER, RouterBuilderRuntimeExt, link},
    view::{View, attributes, view},
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

/// How long the browser runtime may reuse a prefetched page (its cap is 30s).
const RUNTIME_CACHE_CONTROL: HeaderValue = HeaderValue::from_static("private, max-age=30");

/// Whether this request arrived as a runtime page render (a `link` navigation,
/// its prefetch, or a rerun). `RuntimeLayer` rewrites those POSTs to a GET and
/// drops the header, so read the request as it arrived.
fn runtime_render(cx: &Cx) -> bool {
    original_method(cx) == Method::POST
        && original_parts(cx)
            .headers
            .get(&RUNTIME_HEADER)
            .is_some_and(|value| value == "true")
}

#[layout]
async fn root_layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    // Without Cache-Control the runtime reuses a prefetch for 30s, but Vercel
    // then adds `max-age=0` and every prefetch is discarded. Document GETs keep
    // no explicit header.
    if runtime_render(cx) {
        response_headers(cx).append(header::CACHE_CONTROL, RUNTIME_CACHE_CONTROL);
    }
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
            link(
                href: "/products",
                attrs: attributes! {
                    class="mt-8 inline-block rounded-lg bg-white px-6 py-3 text-sm font-semibold text-indigo-700 hover:bg-indigo-50"
                },
                "Browse all products"
            )
        </section>
        <section class="mt-12">
            <div class="flex items-baseline justify-between">
                <h2 class="text-2xl font-bold tracking-tight">"Featured products"</h2>
                link(
                    href: "/products",
                    attrs: attributes! {
                        class="text-sm font-medium text-indigo-600 hover:text-indigo-500"
                    },
                    "View all"
                )
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
        header::{CACHE_CONTROL, CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE},
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

    /// A runtime render (navigation, prefetch, or rerun) may be reused by the
    /// browser for 30s; a plain document GET keeps no explicit Cache-Control.
    #[tokio::test]
    async fn only_runtime_renders_carry_cache_control() {
        let router = test_router();
        for path in ["/", "/products", "/products/1", "/cart"] {
            let request = Request::builder()
                .method("POST")
                .uri(path)
                .header(CONTENT_TYPE, "application/json")
                .header(&RUNTIME_HEADER, "true")
                .body(Body::from("{}"))
                .unwrap();
            let (response, html) = send(&router, request).await;
            assert_eq!(response.status(), StatusCode::OK, "{path}: {html}");
            assert_eq!(
                response
                    .headers()
                    .get_all(CACHE_CONTROL)
                    .iter()
                    .collect::<Vec<_>>(),
                ["private, max-age=30"],
                "{path}"
            );

            let request = Request::builder().uri(path).body(Body::empty()).unwrap();
            let (response, html) = send(&router, request).await;
            assert_eq!(response.status(), StatusCode::OK, "{path}: {html}");
            assert!(
                response.headers().get(CACHE_CONTROL).is_none(),
                "{path}: {:?}",
                response.headers()
            );
        }
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

    /// The `<img>` tags of `html`, in document order.
    fn img_tags(html: &str) -> Vec<&str> {
        html.match_indices("<img ")
            .map(|(at, _)| &html[at..at + html[at..].find('>').unwrap()])
            .collect()
    }

    /// Every cart row gets its product's image, loaded eagerly at 80px.
    #[tokio::test]
    async fn cart_rows_show_their_thumbnails_eagerly() {
        let router = test_router();
        let (_, cookie) = post(&router, "/cart/add", "product_id=1", None).await;
        let (_, cookie) = post(&router, "/cart/add", "product_id=42", cookie.as_deref()).await;
        let html = get(&router, "/cart", cookie.as_deref()).await;
        let catalog = Catalog::load();
        let tags = img_tags(&html);
        assert_eq!(tags.len(), 2, "{html}");
        for (tag, id) in tags.into_iter().zip([1, 42]) {
            let product = catalog.get(id).unwrap();
            let stem = format!(
                "https://assets.test/{}-{}",
                product.category_slug,
                id as usize % crate::images::VARIANTS + 1
            );
            assert!(tag.contains(&format!("src=\"{stem}-640.webp\"")), "{tag}");
            assert!(tag.contains(&format!("{stem}-320.webp 320w, ")), "{tag}");
            assert!(tag.contains(&format!("{stem}-1600.webp 1600w\"")), "{tag}");
            assert!(tag.contains("sizes=\"80px\""), "{tag}");
            assert!(tag.contains("loading=\"eager\""), "{tag}");
            assert!(!tag.contains("fetchpriority"), "{tag}");
        }
    }

    /// Only the likely LCP image is fetched eagerly with high priority:
    /// the first card of the list and the detail page's main photo.
    #[tokio::test]
    async fn only_the_lcp_candidate_gets_high_priority() {
        let router = test_router();
        let html = get(&router, "/products", None).await;
        let tags = img_tags(&html);
        assert!(tags.len() > 1, "{html}");
        assert!(tags[0].contains("fetchpriority=\"high\""), "{}", tags[0]);
        assert!(tags[0].contains("loading=\"eager\""), "{}", tags[0]);
        for tag in &tags[1..] {
            assert!(!tag.contains("fetchpriority"), "{tag}");
            assert!(tag.contains("loading=\"lazy\""), "{tag}");
        }
        for tag in &tags {
            assert!(
                tag.contains(&format!(
                    "sizes=\"{}\"",
                    crate::app::_components::GRID_IMAGE_SIZES
                )),
                "{tag}"
            );
            assert!(tag.contains("width=\"1600\" height=\"1600\""), "{tag}");
        }

        let html = get(&router, "/", None).await;
        assert!(!html.contains("fetchpriority"), "{html}");

        let html = get(&router, "/products/1", None).await;
        let main = img_tags(&html)[0];
        assert!(main.contains("fetchpriority=\"high\""), "{main}");
        assert!(main.contains("sizes=\"(min-width: 1152px) 540px"), "{main}");
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

    #[tokio::test(start_paused = true)]
    async fn the_detail_page_shows_the_current_stock_and_finishes() {
        let router = test_router();
        let before = crate::stock::stock_at(1, std::time::SystemTime::now());
        // The HTTP render emits once and ends; it must not wait for the
        // next interval (with paused time, a wait would surface as a jump).
        let started = tokio::time::Instant::now();
        let html = get(&router, "/products/1", None).await;
        let after = crate::stock::stock_at(1, std::time::SystemTime::now());
        assert!(started.elapsed() < crate::stock::STOCK_INTERVAL);
        assert!(
            html.contains("::topcoat::shard::start(\"/_shards/stock-level\""),
            "{html}"
        );
        // The shard asks the browser for a connected (WebSocket) render.
        assert!(html.contains("<!--::topcoat::connect-->"), "{html}");
        let shown = |level| match level {
            0 => "Out of stock".to_owned(),
            level if level <= crate::stock::LOW_STOCK => format!("Only {level} left in stock"),
            level => format!("{level} in stock"),
        };
        // A slot boundary may pass during the request.
        assert!(
            html.contains(&shown(before)) || html.contains(&shown(after)),
            "{html}"
        );
    }

    /// Pulls every `data-topcoat-on:*` / `data-topcoat-bind:*` attribute out
    /// of `html` as `(name, decoded JavaScript source)`.
    fn runtime_expressions(html: &str) -> Vec<(String, String)> {
        let mut found = Vec::new();
        let mut rest = html;
        while let Some(start) = rest.find(" data-topcoat-") {
            rest = &rest[start + 1..];
            let Some(eq) = rest.find("=\"") else { break };
            let name = &rest[..eq];
            let value_start = eq + 2;
            let Some(len) = rest[value_start..].find('"') else {
                break;
            };
            let value = &rest[value_start..value_start + len];
            if name.starts_with("data-topcoat-on:") || name.starts_with("data-topcoat-bind:") {
                let source = value
                    .replace("&quot;", "\"")
                    .replace("&#39;", "'")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&amp;", "&");
                found.push((name.to_owned(), source));
            }
            rest = &rest[value_start + len..];
        }
        found
    }

    /// The browser compiles each runtime expression with
    /// `new Function("cx", "return " + source + ";")` (`expression/compile.ts`
    /// in the runtime). Rust type-checks the expression but never parses the
    /// JavaScript it lowers to, so a syntax error there only shows up in the
    /// browser. This runs the same wrapper through `node --check`.
    fn assert_compiles_as_javascript(page: &str, expressions: &[(String, String)]) {
        use std::{io::Write, process::Command};

        let Ok(version) = Command::new("node").arg("--version").output() else {
            eprintln!("skipping the JavaScript syntax check: node is not installed");
            return;
        };
        assert!(version.status.success());

        let dir = std::env::temp_dir().join(format!("topcoat-js-check-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (index, (name, source)) in expressions.iter().enumerate() {
            let path = dir.join(format!(
                "{}-{index}.js",
                page.trim_matches('/').replace('/', "_")
            ));
            let mut file = std::fs::File::create(&path).unwrap();
            writeln!(file, "(function (cx) {{ return {source}; }});").unwrap();
            drop(file);
            let output = Command::new("node")
                .arg("--check")
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{page}: {name} is not valid JavaScript:\n{source}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn runtime_expressions_compile_as_javascript() {
        let router = test_router();
        for page in ["/products", "/products?q=lamp", "/products/1", "/cart", "/"] {
            let html = get(&router, page, None).await;
            let expressions = runtime_expressions(&html);
            if page.starts_with("/products") && !page.starts_with("/products/") {
                assert!(
                    expressions
                        .iter()
                        .any(|(name, _)| name == "data-topcoat-on:input"),
                    "{page}: the search handler is missing: {expressions:?}"
                );
            }
            assert_compiles_as_javascript(page, &expressions);
        }
    }

    /// Runtime navigation posts the current page's signal values to the
    /// destination (`navigation.ts` / `page.ts` `signalBody`). The search
    /// signal mirrors `q`, so a link without `q` (header "Products", footer
    /// categories) must not inherit the query typed on the previous page.
    #[tokio::test]
    async fn runtime_navigation_does_not_carry_the_search_to_a_url_without_it() {
        let router = test_router();
        let html = get(&router, "/products?q=lamp", None).await;
        let marker = "::topcoat::signal({&quot;t&quot;:&quot;signal&quot;,&quot;id&quot;:&quot;";
        let at = html.find(marker).expect("the search signal is declared") + marker.len();
        let id: String = html[at..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        assert_eq!(id.len(), 32, "{html}");

        // What the runtime sends when following a link from that page.
        let navigate = |path: &str| {
            Request::builder()
                .method("POST")
                .uri(path)
                .header(CONTENT_TYPE, "application/json")
                .header("x-topcoat-runtime", "true")
                .body(Body::from(format!(r#"{{"signals":{{"{id}":"lamp"}}}}"#)))
                .unwrap()
        };
        let (response, html) = send(&router, navigate("/products")).await;
        assert_eq!(response.status(), StatusCode::OK, "{html}");
        assert!(html.contains("500 products"), "{html}");
        assert!(!html.contains("matching"), "{html}");

        // A link that carries the query still shows its results.
        let (_, html) = send(&router, navigate("/products?category=audio&q=pro")).await;
        assert!(html.contains("matching \u{201c}pro\u{201d}"), "{html}");
    }

    /// Every anchor opts into runtime navigation with the default prefetch
    /// mode, including streamed sections (related products) and the cart.
    #[tokio::test]
    async fn links_use_runtime_navigation_with_intent_prefetch() {
        let router = test_router();
        let (_, cookie) = post(&router, "/cart/add", "product_id=1", None).await;
        for page in ["/", "/products?page=2", "/products/1", "/cart"] {
            let html = get(&router, page, cookie.as_deref()).await;
            let anchors: Vec<&str> = html
                .match_indices("<a ")
                .map(|(at, _)| &html[at..at + html[at..].find('>').unwrap()])
                .collect();
            assert!(anchors.len() > 10, "{page}: {html}");
            for anchor in anchors {
                assert!(
                    anchor.contains(r#"data-topcoat-link="intent""#),
                    "{page}: {anchor}"
                );
            }
        }
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
