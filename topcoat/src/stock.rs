//! Simulated live stock levels. A level is a pure function of the product ID
//! and the current time slot, so every instance reports the same number at
//! the same moment without sharing state (given synchronized clocks).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How often the level changes. Every client wakes on the same boundaries.
pub const STOCK_INTERVAL: Duration = Duration::from_secs(5);

/// At or below this level the badge warns that stock is low.
pub const LOW_STOCK: u32 = 5;

/// The stock level of `product_id` during the slot containing `at`.
pub fn stock_at(product_id: u32, at: SystemTime) -> u32 {
    let slot = slot_of(at);
    // Each product has its own baseline (5..=44) and drifts around it by
    // up to ±6 from one slot to the next. Some slots dip to zero.
    let baseline = mix(u64::from(product_id)) % 40 + 5;
    let drift = mix(u64::from(product_id) << 32 ^ slot) % 13;
    u32::try_from((baseline + drift).saturating_sub(6)).unwrap_or(u32::MAX)
}

/// Time left until the next slot boundary after `at`.
pub fn until_next_change(at: SystemTime) -> Duration {
    let since_epoch = at.duration_since(UNIX_EPOCH).unwrap_or_default();
    let interval = STOCK_INTERVAL.as_millis();
    let into_slot = since_epoch.as_millis() % interval;
    Duration::from_millis(u64::try_from(interval - into_slot).unwrap_or(0))
}

fn slot_of(at: SystemTime) -> u64 {
    let since_epoch = at.duration_since(UNIX_EPOCH).unwrap_or_default();
    since_epoch.as_secs() / STOCK_INTERVAL.as_secs()
}

/// `SplitMix64` finalizer: a cheap, well-spread integer hash.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn the_level_is_fixed_within_a_slot_and_varies_across_slots() {
        let base = 1_800_000_000;
        assert_eq!(stock_at(1, at(base)), stock_at(1, at(base + 4)));
        let levels: Vec<u32> = (0..50).map(|n| stock_at(1, at(base + n * 5))).collect();
        assert!(
            levels.windows(2).any(|pair| pair[0] != pair[1]),
            "{levels:?}"
        );
        assert!(levels.iter().all(|level| *level <= 50), "{levels:?}");
    }

    #[test]
    fn products_differ() {
        let now = at(1_800_000_000);
        let levels: Vec<u32> = (1..=20).map(|id| stock_at(id, now)).collect();
        assert!(levels.iter().any(|level| *level != levels[0]), "{levels:?}");
    }

    #[test]
    fn the_next_change_lands_on_a_slot_boundary() {
        let start = UNIX_EPOCH + Duration::from_millis(1_800_000_001_250);
        let wait = until_next_change(start);
        assert_eq!(wait, Duration::from_millis(3_750));
        assert_ne!(slot_of(start), slot_of(start + wait));
    }
}
