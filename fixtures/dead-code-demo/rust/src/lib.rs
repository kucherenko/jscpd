//! A label press: takes an order, prints the shipping labels it needs.

use std::collections::HashMap;

mod layout;

/// Public, so no lint ever fires for it: another crate may call it.
pub fn labels_for(order_id: u32, parcels: u32) -> Vec<String> {
    (1..=parcels)
        .map(|n| layout::render(order_id, n, parcels))
        .collect()
}

/// Written for a "reprint" button that was never wired up.
fn reprint(order_id: u32) -> Vec<String> {
    labels_for(order_id, 1)
}

struct Roll {
    width_mm: u32,
    remaining: u32,
}

impl Roll {
    fn fits(&self, count: u32) -> bool {
        self.remaining >= count && self.width_mm >= 100
    }
}

const MAX_PER_ROLL: u32 = 500;
