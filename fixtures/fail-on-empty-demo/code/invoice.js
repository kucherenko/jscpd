// Invoice totals for the demo shop. The discount loop is a copy of the one in
// pricing.js, which is the clone the demo commands report.

export function invoiceLine(quantity, unitPrice, sku) {
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
  return { subtotal, discount: 0, total: subtotal, tier: null, sku };
}

export function formatLine(line) {
  return `${line.sku}: ${line.total.toFixed(2)}`;
}
