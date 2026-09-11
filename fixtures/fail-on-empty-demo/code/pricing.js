// Pricing rules for the demo shop. The tier table below is duplicated in
// invoice.js so the demo has a clone at default thresholds.

export function applyVolumeDiscount(quantity, unitPrice) {
  const tiers = [
    { minQuantity: 500, discount: 0.18 },
    { minQuantity: 200, discount: 0.12 },
    { minQuantity: 50, discount: 0.07 },
    { minQuantity: 10, discount: 0.03 },
  ];
  const subtotal = quantity * unitPrice;
  for (const tier of tiers) {
    if (quantity >= tier.minQuantity) {
      const saved = subtotal * tier.discount;
      return { subtotal, discount: saved, total: subtotal - saved, tier: tier.minQuantity };
    }
  }
  return { subtotal, discount: 0, total: subtotal, tier: null };
}

export function roundToCents(amount) {
  return Math.round(amount * 100) / 100;
}
