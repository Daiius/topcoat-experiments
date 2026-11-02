use topcoat::{
    Result,
    asset::asset_config,
    context::Cx,
    view::{PromotedStr, StaticClass, View, class, component, view},
};

use crate::{
    cart::cart,
    catalog::{Product, Review, Spec, filled_stars, format_rating},
    images::{IMAGE_SIZE, product_image},
    urls::products_url,
};

/// The heroicons solid star, shared verbatim with the Next.js version.
const STAR_PATH: PromotedStr = PromotedStr(
    &"M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.539 1.118l-2.8-2.034a1 1 0 00-1.176 0l-2.8 2.034c-.783.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.363-1.118l-2.8-2.034c-.784-.57-.381-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z",
);

const PAGE_LINK: StaticClass =
    class!("rounded-md px-3 py-2 font-medium text-slate-600 hover:bg-slate-100");
const PAGE_DISABLED: StaticClass = class!("rounded-md px-3 py-2 font-medium text-slate-300");
const PAGE_CURRENT: StaticClass =
    class!("rounded-md bg-indigo-600 px-3 py-2 font-semibold text-white");

const FOOTER_COLUMNS: [(&str, [(&str, &str); 4]); 4] = [
    (
        "Shop",
        [
            ("All products", "/products"),
            ("Audio", "/products?category=audio"),
            ("Displays", "/products?category=displays"),
            ("Wearables", "/products?category=wearables"),
        ],
    ),
    (
        "Support",
        [
            ("Contact", "#"),
            ("Shipping", "#"),
            ("Returns", "#"),
            ("Warranty", "#"),
        ],
    ),
    (
        "Company",
        [
            ("About", "#"),
            ("Careers", "#"),
            ("Press", "#"),
            ("Sustainability", "#"),
        ],
    ),
    (
        "Legal",
        [
            ("Privacy", "#"),
            ("Terms", "#"),
            ("Imprint", "#"),
            ("Cookie settings", "#"),
        ],
    ),
];

#[component]
pub async fn site_nav(cx: &Cx) -> Result<impl View> {
    // Read on every request: the count comes from the visitor's cookie.
    let count = cart(cx).read().count();

    Ok(view! {
        <header class="border-b border-slate-200 bg-white">
            <nav
                class="mx-auto flex w-full max-w-6xl items-center justify-between px-4 py-4"
            >
                <a href="/" class="text-lg font-bold tracking-tight">
                    "Meridian Supply"
                </a>
                <div class="flex items-center gap-6 text-sm font-medium text-slate-600">
                    <a href="/" class="hover:text-slate-900">"Home"</a>
                    <a href="/products" class="hover:text-slate-900">"Products"</a>
                    <a
                        href="/cart"
                        class="rounded-full bg-indigo-600 px-3 py-1 text-xs font-semibold text-white hover:bg-indigo-500"
                    >
                        "Cart ("
                        (count)
                        ")"
                    </a>
                </div>
            </nav>
        </header>
    })
}

#[component]
pub async fn site_footer() -> Result<impl View> {
    Ok(view! {
        <footer class="border-t border-slate-200 bg-white">
            <div
                class="mx-auto grid w-full max-w-6xl grid-cols-2 gap-8 px-4 py-10 text-sm md:grid-cols-4"
            >
                for (title, links) in FOOTER_COLUMNS {
                    <div>
                        <h3 class="mb-3 font-semibold text-slate-900">(title)</h3>
                        <ul class="space-y-2 text-slate-500">
                            for (label, href) in links {
                                <li>
                                    <a href=(href) class="hover:text-slate-900">(label)</a>
                                </li>
                            }
                        </ul>
                    </div>
                }
            </div>
            <div class="border-t border-slate-100">
                <p class="mx-auto w-full max-w-6xl px-4 py-4 text-xs text-slate-400">
                    "(c) 2026 Meridian Supply. All rights reserved."
                </p>
            </div>
        </footer>
    })
}

#[component]
pub async fn rating_stars(tenths: u32, size: &str) -> Result<impl View> {
    let filled = filled_stars(tenths);

    Ok(view! {
        <div class="flex">
            for index in 0..5u32 {
                <svg
                    viewBox="0 0 20 20"
                    fill="currentColor"
                    aria-hidden="true"
                    class=(class!(
                        size,
                        "text-amber-400" if index < filled else "text-slate-200",
                    ))
                >
                    <path d=(STAR_PATH)></path>
                </svg>
            }
        </div>
    })
}

/// `sizes` defaults to the product grid's; `priority` marks the likely LCP card.
#[component]
pub async fn product_card(
    product: &Product,
    #[default(GRID_IMAGE_SIZES)] sizes: &str,
    #[default] priority: bool,
) -> Result<impl View> {
    Ok(view! {
        <a
            href=(("/products/", product.id))
            class="group flex flex-col rounded-xl border border-slate-200 bg-white p-4 shadow-sm transition hover:shadow-md"
        >
            product_photo(
                product: product,
                frame: "mb-4 h-32 rounded-lg",
                sizes: sizes,
                priority: priority
            )
            <p class="text-xs font-medium uppercase tracking-wide text-slate-400">
                (&product.category)
            </p>
            <h3
                class="mt-1 text-sm font-semibold text-slate-900 group-hover:text-indigo-600"
            >
                (&product.name)
            </h3>
            <div class="mt-2 flex items-center gap-1">
                rating_stars(tenths: product.rating_tenths, size: "h-4 w-4")
                <span class="text-xs text-slate-500">
                    (format_rating(product.rating_tenths))
                    " ("
                    (product.review_count)
                    ")"
                </span>
            </div>
            <p class="mt-3 text-lg font-bold">(product.price())</p>
        </a>
    })
}

/// `sizes` for a card in the 2 / 3 / 4 column product grid (72rem container),
/// as in the Next.js version.
pub const GRID_IMAGE_SIZES: &str = "(min-width: 1024px) 262px, (min-width: 768px) 33vw, 50vw";

/// `sizes` for a card in the 2 / 4 column related-products grid.
pub const RELATED_IMAGE_SIZES: &str = "(min-width: 1024px) 262px, (min-width: 768px) 25vw, 50vw";

/// A product's image from pre-resized files: the browser picks one from
/// `srcset` by `sizes`, which has to describe the box `frame` gives it.
/// `frame` sizes the box; the image covers it. `priority` marks the likely
/// LCP image (eager, `fetchpriority="high"`); `eager` only skips lazy loading.
/// Falls back to the initials placeholder.
#[component]
pub async fn product_photo(
    cx: &Cx,
    product: &Product,
    frame: &str,
    sizes: &str,
    #[default] eager: bool,
    #[default] priority: bool,
) -> Result<impl View> {
    // `srcset` lists URLs, so resolve them here rather than in `view!`.
    let image = product_image(product).map(|image| {
        let config = asset_config(cx);
        let srcset = image
            .widths()
            .map(|(asset, width)| format!("{} {width}w", config.resolve(asset)))
            .collect::<Vec<_>>()
            .join(", ");
        (image.fallback(), srcset)
    });

    Ok(view! {
        if let Some((src, srcset)) = image {
            <img
                src=(src)
                srcset=(srcset)
                sizes=(sizes)
                alt=(&product.name)
                width=(IMAGE_SIZE)
                height=(IMAGE_SIZE)
                loading=(if eager || priority { "eager" } else { "lazy" })
                fetchpriority=(priority.then_some("high"))
                decoding="async"
                class=(class!(frame, "w-full bg-slate-100 object-cover"))
            >
        } else {
            <div
                class=(class!(
                    frame,
                    "flex items-center justify-center bg-slate-100 text-3xl font-bold text-slate-300",
                ))
            >
                (product.initials())
            </div>
        }
    })
}

#[component]
pub async fn pagination(
    current: usize,
    page_count: usize,
    sort: Option<&str>,
    category: Option<&str>,
    search: Option<&str>,
) -> Result<impl View> {
    Ok(view! {
        <nav
            aria-label="Pagination"
            class="mt-10 flex flex-wrap items-center justify-center gap-1 text-sm"
        >
            if current > 1 {
                <a
                    href=(products_url(current - 1, sort, category, search))
                    class=(PAGE_LINK)
                >
                    "Previous"
                </a>
            } else {
                <span class=(PAGE_DISABLED)>"Previous"</span>
            }
            for number in 1..=page_count {
                if number == current {
                    <span aria-current="page" class=(PAGE_CURRENT)>(number)</span>
                } else {
                    <a
                        href=(products_url(number, sort, category, search))
                        class=(PAGE_LINK)
                    >
                        (number)
                    </a>
                }
            }
            if current < page_count {
                <a
                    href=(products_url(current + 1, sort, category, search))
                    class=(PAGE_LINK)
                >
                    "Next"
                </a>
            } else {
                <span class=(PAGE_DISABLED)>"Next"</span>
            }
        </nav>
    })
}

#[component]
pub async fn breadcrumbs(category: &str, category_slug: &str, name: &str) -> Result<impl View> {
    Ok(view! {
        <nav aria-label="Breadcrumb" class="text-sm text-slate-500">
            <ol class="flex flex-wrap items-center gap-2">
                <li><a href="/" class="hover:text-slate-900">"Home"</a></li>
                <li>"/"</li>
                <li><a href="/products" class="hover:text-slate-900">"Products"</a></li>
                <li>"/"</li>
                <li>
                    <a
                        href=(("/products?category=", category_slug))
                        class="hover:text-slate-900"
                    >
                        (category)
                    </a>
                </li>
                <li>"/"</li>
                <li class="font-medium text-slate-900">(name)</li>
            </ol>
        </nav>
    })
}

#[component]
pub async fn spec_table(specs: &[Spec]) -> Result<impl View> {
    Ok(view! {
        <div class="mt-6 overflow-hidden rounded-xl border border-slate-200 bg-white">
            <table class="w-full text-sm">
                <tbody>
                    for spec in specs {
                        <tr class="border-b border-slate-100 last:border-0">
                            <th
                                scope="row"
                                class="w-1/3 px-4 py-3 text-left font-medium text-slate-500"
                            >
                                (&spec.key)
                            </th>
                            <td class="px-4 py-3 text-slate-900">(&spec.value)</td>
                        </tr>
                    }
                </tbody>
            </table>
        </div>
    })
}

#[component]
pub async fn review_list(reviews: &[Review]) -> Result<impl View> {
    Ok(view! {
        <div class="mt-6 space-y-6">
            for review in reviews {
                <article class="rounded-xl border border-slate-200 bg-white p-6">
                    <div class="flex flex-wrap items-center justify-between gap-2">
                        <p class="font-semibold text-slate-900">(&review.author)</p>
                        <p class="text-xs text-slate-400">(&review.date)</p>
                    </div>
                    <div class="mt-2 flex items-center gap-2">
                        rating_stars(tenths: review.rating_tenths, size: "h-4 w-4")
                        <p class="text-sm font-medium text-slate-700">
                            (&review.title)
                        </p>
                    </div>
                    <p class="mt-3 text-sm text-slate-600">(&review.body)</p>
                </article>
            }
        </div>
    })
}

/// Placeholder for `review_list` while reviews load. Matches the card layout
/// so the page does not jump when the content replaces it.
#[component]
pub async fn review_list_skeleton(count: u32) -> Result<impl View> {
    Ok(view! {
        <div class="mt-6 space-y-6" aria-busy="true" aria-label="Loading reviews">
            for _ in 0..count {
                <div
                    class="animate-pulse rounded-xl border border-slate-200 bg-white p-6"
                >
                    <div class="h-4 w-1/4 rounded bg-slate-200"></div>
                    <div class="mt-3 h-4 w-1/2 rounded bg-slate-200"></div>
                    <div class="mt-4 h-3 w-full rounded bg-slate-100"></div>
                    <div class="mt-2 h-3 w-5/6 rounded bg-slate-100"></div>
                </div>
            }
        </div>
    })
}

/// Placeholder for `product_card` while recommendations load.
#[component]
pub async fn product_card_skeleton() -> Result<impl View> {
    Ok(view! {
        <div
            class="flex animate-pulse flex-col rounded-xl border border-slate-200 bg-white p-4 shadow-sm"
            aria-hidden="true"
        >
            <div class="mb-4 h-32 rounded-lg bg-slate-100"></div>
            <div class="h-3 w-1/3 rounded bg-slate-100"></div>
            <div class="mt-2 h-4 w-3/4 rounded bg-slate-200"></div>
            <div class="mt-3 h-4 w-1/2 rounded bg-slate-100"></div>
            <div class="mt-3 h-5 w-1/4 rounded bg-slate-200"></div>
        </div>
    })
}
