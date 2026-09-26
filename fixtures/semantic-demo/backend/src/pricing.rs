//! Order pricing. Every amount is an integer number of cents.

use serde::{Deserialize, Serialize};

const TAX_RATE_PERCENT: i64 = 20;
const FREE_SHIPPING_FROM: i64 = 5_000;
const SHIPPING_FEE: i64 = 499;

#[derive(Debug, Deserialize)]
pub struct CartLine {
    pub sku: String,
    pub unit_price_cents: i64,
    pub quantity: u32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Coupon {
    Percent { percent: u8 },
    Fixed { amount_cents: i64 },
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Totals {
    pub subtotal: i64,
    pub discount: i64,
    pub tax: i64,
    pub shipping: i64,
    pub total: i64,
}

/// Subtotal, coupon discount, VAT on the discounted amount and shipping,
/// which is free above the threshold and for an empty cart.
pub fn cart_totals(lines: &[CartLine], coupon: Option<&Coupon>) -> Totals {
    let subtotal: i64 = lines
        .iter()
        .map(|line| line.unit_price_cents * i64::from(line.quantity))
        .sum();
    let discount = match coupon {
        Some(Coupon::Percent { percent }) => subtotal * i64::from((*percent).min(100)) / 100,
        Some(Coupon::Fixed { amount_cents }) => (*amount_cents).clamp(0, subtotal),
        None => 0,
    };
    let taxable = subtotal - discount;
    let tax = (taxable * TAX_RATE_PERCENT + 50) / 100;
    let shipping = if lines.is_empty() || taxable >= FREE_SHIPPING_FROM {
        0
    } else {
        SHIPPING_FEE
    };
    Totals {
        subtotal,
        discount,
        tax,
        shipping,
        total: taxable + tax + shipping,
    }
}

/// Stock left for each SKU after the order lines are reserved; `None` when a
/// line asks for more than is in stock.
pub fn reserve_stock(
    stock: &std::collections::HashMap<String, u32>,
    lines: &[CartLine],
) -> Option<std::collections::HashMap<String, u32>> {
    let mut left = stock.clone();
    for line in lines {
        let available = left.get_mut(&line.sku)?;
        if *available < line.quantity {
            return None;
        }
        *available -= line.quantity;
    }
    Some(left)
}
