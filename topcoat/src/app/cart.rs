pub(crate) mod add;
mod update;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{href, page},
    view::{View, view},
};

use crate::{
    app::_components::product_photo,
    cart::{MAX_QUANTITY, cart, format_cents, total_cents},
    catalog::Catalog,
};

#[page]
async fn cart_page(cx: &Cx) -> Result<impl View> {
    let catalog = app_context::<Catalog>(cx);
    let items = cart(cx).read().items(catalog);
    let total = format_cents(total_cents(&items));
    let count: u32 = items.iter().map(|(_, quantity)| quantity).sum();

    Ok(view! {
        <h1 class="text-3xl font-bold tracking-tight">"Your cart"</h1>
        if items.is_empty() {
            <div
                class="mt-8 rounded-xl border border-slate-200 bg-white p-8 text-center"
            >
                <p class="text-slate-500">"Your cart is empty."</p>
                <a
                    href="/products"
                    class="mt-4 inline-block text-sm font-medium text-indigo-600 hover:text-indigo-500"
                >
                    "Browse products"
                </a>
            </div>
        } else {
            <ul
                class="mt-8 divide-y divide-slate-100 rounded-xl border border-slate-200 bg-white"
            >
                for (product, quantity) in &items {
                    <li class="flex flex-wrap items-center gap-4 p-4">
                        <div class="w-20 shrink-0">
                            product_photo(
                                product: product,
                                frame: "h-20 rounded-lg",
                                eager: false
                            )
                        </div>
                        <div class="min-w-0 flex-1">
                            <a
                                href=(("/products/", product.id))
                                class="font-semibold text-slate-900 hover:text-indigo-600"
                            >
                                (&product.name)
                            </a>
                            <p class="text-sm text-slate-500">
                                (product.price())
                                " each"
                            </p>
                        </div>
                        // Each change is an ordinary form POST answered with a
                        // redirect back here, so it works without JavaScript.
                        <form
                            method="post"
                            action=(href!(update::update))
                            class="flex items-center gap-2"
                        >
                            <input type="hidden" name="product_id" value=(product.id)>
                            <label class="sr-only" for=(("quantity-", product.id))>
                                "Quantity"
                            </label>
                            <input
                                id=(("quantity-", product.id))
                                type="number"
                                name="quantity"
                                min="0"
                                max=(MAX_QUANTITY)
                                value=(quantity)
                                class="w-20 rounded-lg border border-slate-300 px-2 py-1 text-sm"
                            >
                            <button
                                type="submit"
                                class="rounded-lg border border-slate-300 px-3 py-1 text-sm font-medium hover:bg-slate-50"
                            >
                                "Update"
                            </button>
                        </form>
                        <form method="post" action=(href!(update::update))>
                            <input type="hidden" name="product_id" value=(product.id)>
                            <input type="hidden" name="quantity" value="0">
                            <button
                                type="submit"
                                class="text-sm font-medium text-rose-600 hover:text-rose-500"
                            >
                                "Remove"
                            </button>
                        </form>
                    </li>
                }
            </ul>
            <div class="mt-6 flex items-baseline justify-end gap-4">
                <p class="text-sm text-slate-500">
                    (count)
                    " items"
                </p>
                <p class="text-2xl font-bold">(total)</p>
            </div>
        }
    })
}
