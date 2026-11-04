//! Stand-ins for slow upstream services (a reviews API, a recommendation
//! engine). Each call sleeps for a fixed delay, then reads the bundled
//! catalog. Nothing is mutated, so every instance answers alike. Reviews are
//! fetched on every request; recommendations are cached per instance.

use std::{sync::Arc, time::Duration};

use cached::macros::cached;

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
    fetch_related_ids(catalog, product_id)
        .await
        .iter()
        .filter_map(|id| catalog.get(*id))
        .collect()
}

/// Asks the recommendation service, at most once per product for the life of
/// the process: the counterpart of Next.js `'use cache'` + `cacheLife('max')`
/// on `fetchRelated` (the catalog never changes, so there is no expiry).
///
/// The cache is a static in this process, so each instance starts cold and
/// the first visit to a product still waits. It holds IDs rather than
/// `&Product`, keyed by `product_id` alone; concurrent misses for the same
/// product share one fetch (`sync_writes = "by_key"`).
#[cached(key = "u32", convert = "{ product_id }", sync_writes = "by_key")]
async fn fetch_related_ids(catalog: &Catalog, product_id: u32) -> Arc<[u32]> {
    tokio::time::sleep(RELATED_DELAY).await;
    catalog
        .get(product_id)
        .map(|product| product.related_ids.as_slice().into())
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

    // The related-products cache is one static shared by every test in the
    // process, so each test that times `fetch_related` uses its own product
    // (the page tests in `app` visit product 1, `app::products::id` uses 4).

    #[tokio::test(start_paused = true)]
    async fn related_products_arrive_after_their_delay() {
        let catalog = Catalog::load();
        let started = Instant::now();
        let related = fetch_related(&catalog, 2).await;
        assert_eq!(started.elapsed(), RELATED_DELAY);
        let expected = &catalog.get(2).unwrap().related_ids;
        let ids: Vec<u32> = related.iter().map(|product| product.id).collect();
        assert_eq!(&ids, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn related_products_are_fetched_once_per_product() {
        let catalog = Catalog::load();
        let ids = |related: Vec<&Product>| -> Vec<u32> {
            related.iter().map(|product| product.id).collect()
        };
        let started = Instant::now();
        let first = ids(fetch_related(&catalog, 3).await);
        assert_eq!(started.elapsed(), RELATED_DELAY);

        // The second visit is served from the cache, without the delay.
        let started = Instant::now();
        let second = ids(fetch_related(&catalog, 3).await);
        assert_eq!(started.elapsed(), Duration::ZERO);
        assert_eq!(second, first);
        assert_eq!(&second, &catalog.get(3).unwrap().related_ids);
    }

    #[tokio::test(start_paused = true)]
    async fn unknown_products_have_nothing() {
        let catalog = Catalog::load();
        assert!(fetch_reviews(&catalog, 0).await.is_empty());
        assert!(fetch_related(&catalog, 0).await.is_empty());
    }
}
