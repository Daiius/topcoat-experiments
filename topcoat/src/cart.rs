//! The shopping cart, kept entirely in a signed cookie so any instance can
//! serve any request. The server stores nothing.

use serde::{Deserialize, Serialize};
use topcoat::{
    context::Cx,
    cookie::{CookieStore, Cookies, Key, SameSite, cookie_store, signed_cookies},
};

use crate::catalog::{Catalog, Product};

/// Most units of one product a cart line can hold.
pub const MAX_QUANTITY: u32 = 99;

/// Most distinct products a cart can hold. Keeps the cookie far below the
/// browser's 4 KB limit (each line is about 12 bytes of JSON).
pub const MAX_LINES: usize = 50;

/// Environment variable holding the cookie signing key as 128 hex digits
/// (64 bytes). Every instance must share it, or carts signed by one instance
/// are rejected by the others.
pub const KEY_ENV: &str = "COOKIE_KEY";

/// The cart: `(product ID, quantity)` pairs in the order they were added.
/// Serialized compactly as `[[12,1],[7,3]]`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cart(Vec<(u32, u32)>);

impl Cart {
    #[cfg(test)]
    pub fn lines(&self) -> &[(u32, u32)] {
        &self.0
    }

    /// Total units across all lines, as shown in the header.
    pub fn count(&self) -> u32 {
        self.0.iter().map(|(_, quantity)| quantity).sum()
    }

    /// Adds `quantity` units of a product, capped at [`MAX_QUANTITY`]. A new
    /// product is ignored once the cart holds [`MAX_LINES`] products.
    pub fn add(&mut self, product_id: u32, quantity: u32) {
        if let Some((_, current)) = self.0.iter_mut().find(|(id, _)| *id == product_id) {
            *current = current.saturating_add(quantity).min(MAX_QUANTITY);
        } else if self.0.len() < MAX_LINES {
            self.0.push((product_id, quantity.min(MAX_QUANTITY)));
        }
    }

    /// Sets a line's quantity, capped at [`MAX_QUANTITY`]. Zero removes it.
    pub fn set(&mut self, product_id: u32, quantity: u32) {
        if quantity == 0 {
            self.0.retain(|(id, _)| *id != product_id);
        } else if let Some((_, current)) = self.0.iter_mut().find(|(id, _)| *id == product_id) {
            *current = quantity.min(MAX_QUANTITY);
        }
    }

    /// Drops lines whose product no longer exists and repairs quantities.
    /// The cookie is signed, but its contents may come from an older build.
    pub fn sanitize(&mut self, catalog: &Catalog) {
        self.0
            .retain(|(id, quantity)| *quantity > 0 && catalog.get(*id).is_some());
        self.0.truncate(MAX_LINES);
        for (_, quantity) in &mut self.0 {
            *quantity = (*quantity).min(MAX_QUANTITY);
        }
    }

    /// Lines joined with their products.
    pub fn items<'a>(&self, catalog: &'a Catalog) -> Vec<(&'a Product, u32)> {
        self.0
            .iter()
            .filter_map(|(id, quantity)| Some((catalog.get(*id)?, *quantity)))
            .collect()
    }
}

/// Total price of `items`, in cents.
pub fn total_cents(items: &[(&Product, u32)]) -> u64 {
    items
        .iter()
        .map(|(product, quantity)| u64::from(product.price_cents) * u64::from(*quantity))
        .sum()
}

/// Formats cents as dollars, like [`Product::price`].
pub fn format_cents(cents: u64) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

/// The jar every cart access goes through, so the attributes are set once.
fn cookies(cx: &Cx) -> impl Cookies {
    signed_cookies(cx)
        .default_path("/")
        .default_http_only(true)
        .default_same_site(SameSite::Lax)
        .override_secure(true)
}

/// The cart cookie. A missing or tampered cookie reads as an empty cart.
pub fn cart(cx: &Cx) -> CookieStore<Cart, impl Cookies> {
    cookie_store(cookies(cx), "cart").parse_or_default()
}

/// Loads the signing key from [`KEY_ENV`].
///
/// Debug builds fall back to a random key with a warning; it only works for
/// a single instance and resets carts on restart. Release builds refuse to
/// start without the variable, since instances must share one key.
///
/// # Panics
///
/// Panics if the variable is set but is not 128 hex digits, or, in release
/// builds, if it is not set.
pub fn signing_key() -> Key {
    let value = std::env::var(KEY_ENV).ok();
    load_key(value.as_deref(), cfg!(debug_assertions)).unwrap_or_else(|error| panic!("{error}"))
}

fn load_key(value: Option<&str>, allow_random: bool) -> Result<Key, String> {
    match value {
        Some(hex) => {
            let bytes = decode_hex(hex.trim()).ok_or(format!("{KEY_ENV} must be hex-encoded"))?;
            Key::try_from(bytes.as_slice())
                .map_err(|error| format!("{KEY_ENV} is not a usable key: {error}"))
        }
        None if allow_random => {
            eprintln!(
                "warning: {KEY_ENV} is not set; using a random cookie key. \
                 Carts reset on restart and are not shared between instances."
            );
            Ok(Key::generate())
        }
        None => Err(format!(
            "{KEY_ENV} is not set. Release builds need the cookie signing key shared by \
             every instance: 128 hex digits, e.g. from `openssl rand -hex 64`."
        )),
    }
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_merges_lines_and_caps_quantities() {
        let mut cart = Cart::default();
        cart.add(1, 2);
        cart.add(7, 1);
        cart.add(1, 3);
        assert_eq!(cart.lines(), &[(1, 5), (7, 1)]);
        assert_eq!(cart.count(), 6);
        cart.add(7, 500);
        assert_eq!(cart.lines(), &[(1, 5), (7, MAX_QUANTITY)]);
    }

    #[test]
    fn setting_zero_removes_a_line() {
        let mut cart = Cart::default();
        cart.add(1, 2);
        cart.add(2, 1);
        cart.set(1, 4);
        cart.set(2, 0);
        cart.set(3, 9);
        assert_eq!(cart.lines(), &[(1, 4)]);
    }

    #[test]
    fn the_cart_stops_growing_at_the_line_limit() {
        let mut cart = Cart::default();
        for id in 1..=u32::try_from(MAX_LINES).unwrap() + 5 {
            cart.add(id, 1);
        }
        assert_eq!(cart.lines().len(), MAX_LINES);
    }

    #[test]
    fn sanitizing_drops_unknown_products() {
        let catalog = Catalog::load();
        let mut cart = Cart(vec![(1, 2), (9999, 1), (2, 0), (3, 1000)]);
        cart.sanitize(&catalog);
        assert_eq!(cart.lines(), &[(1, 2), (3, MAX_QUANTITY)]);
    }

    #[test]
    fn the_cookie_value_is_compact() {
        let cart = Cart(vec![(12, 1), (7, 3)]);
        assert_eq!(serde_json::to_string(&cart).unwrap(), "[[12,1],[7,3]]");
    }

    #[test]
    fn release_builds_require_the_key() {
        let error = load_key(None, false).err().unwrap();
        assert!(error.contains("COOKIE_KEY is not set"), "{error}");
        assert!(load_key(None, true).is_ok());
    }

    #[test]
    fn a_set_key_must_be_valid() {
        let valid = "ab".repeat(64);
        assert!(load_key(Some(&valid), false).is_ok());
        assert!(load_key(Some("not hex"), true).is_err());
        assert!(load_key(Some(&"ab".repeat(16)), true).is_err());
    }

    #[test]
    fn hex_keys_decode() {
        assert_eq!(decode_hex("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(decode_hex("0"), None);
        assert_eq!(decode_hex("zz"), None);
    }
}
