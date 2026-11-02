use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{RouterErrorExt, SeeOther, see_other},
        href, route,
    },
};

use crate::{cart::cart, catalog::Catalog};

#[derive(Deserialize)]
struct AddForm {
    product_id: u32,
    quantity: Option<u32>,
}

/// Adds a product to the cart and redirects to the cart page. Unknown
/// products answer 404; the form data is user input.
#[route(POST)]
pub async fn add(cx: &Cx, Form(input): Form<AddForm>) -> Result<SeeOther> {
    let catalog = app_context::<Catalog>(cx);
    let product = catalog.get(input.product_id).ok_or_not_found()?;
    let quantity = input.quantity.unwrap_or(1).max(1);
    cart(cx)
        .update(|cart| {
            cart.sanitize(catalog);
            cart.add(product.id, quantity);
        })
        .commit()?;
    Ok(see_other(href!(super::cart_page).resolve(cx)))
}
