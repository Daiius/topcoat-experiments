//! Stand-ins for slow upstream services (a reviews API, a recommendation
//! engine). Each call sleeps for a fixed delay, then reads the bundled
//! catalog. Nothing is cached or mutated, so every instance answers alike.

use std::time::Duration;

use crate::catalog::{Catalog, Product, Review};

/// Simulated latency of the reviews service.
pub const REVIEWS_DELAY: Duration = Duration::from_millis(200);

/// Simulated latency of the recommendation service.
pub const RELATED_DELAY: Duration = Duration::from_millis(800);

/// Fetches the reviews of a product. Unknown IDs have no reviews.
pub async fn fetch_reviews(catalog: &Catalog, product_id: u32) -> &[Review] {
    tokio::time::sleep(REVIEWS_DELAY).await;
    catalog
        .get(product_id)
        .map_or(&[], |product| product.reviews.as_slice())
}

/// Fetches the products recommended alongside a product. Unknown IDs have no
/// recommendations.
pub async fn fetch_related(catalog: &Catalog, product_id: u32) -> Vec<&Product> {
    tokio::time::sleep(RELATED_DELAY).await;
    catalog
        .get(product_id)
        .map(|product| {
            product
                .related_ids
                .iter()
                .filter_map(|id| catalog.get(*id))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use tokio::time::Instant;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn reviews_arrive_after_their_delay() {
        let catalog = Catalog::load();
        let started = Instant::now();
        let reviews = fetch_reviews(&catalog, 1).await;
        assert_eq!(started.elapsed(), REVIEWS_DELAY);
        assert!(!reviews.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn related_products_arrive_after_their_delay() {
        let catalog = Catalog::load();
        let started = Instant::now();
        let related = fetch_related(&catalog, 1).await;
        assert_eq!(started.elapsed(), RELATED_DELAY);
        let expected = &catalog.get(1).unwrap().related_ids;
        let ids: Vec<u32> = related.iter().map(|product| product.id).collect();
        assert_eq!(&ids, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn unknown_products_have_nothing() {
        let catalog = Catalog::load();
        assert!(fetch_reviews(&catalog, 0).await.is_empty());
        assert!(fetch_related(&catalog, 0).await.is_empty());
    }
}
