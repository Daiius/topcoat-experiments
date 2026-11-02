use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{SeeOther, see_other},
        href, route,
    },
};

use crate::{cart::cart, catalog::Catalog};

#[derive(Deserialize)]
struct UpdateForm {
    product_id: u32,
    quantity: u32,
}

/// Sets a cart line's quantity; zero removes it. Answers with a redirect to
/// the cart (post/redirect/get), so reloading does not resubmit.
#[route(POST)]
pub async fn update(cx: &Cx, Form(input): Form<UpdateForm>) -> Result<SeeOther> {
    let catalog = app_context::<Catalog>(cx);
    cart(cx)
        .update(|cart| {
            cart.sanitize(catalog);
            cart.set(input.product_id, input.quantity);
        })
        .commit()?;
    Ok(see_other(href!(super::cart_page).resolve(cx)))
}
