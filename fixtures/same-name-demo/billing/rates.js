// Rates for an invoice line: the tax band of the country, the discount of
// the customer tier and the rounding the ledger expects.
const TAX_BANDS = { de: 0.19, fr: 0.2, pl: 0.23, ua: 0.2 };
const TIER_DISCOUNTS = { bronze: 0, silver: 0.05, gold: 0.1 };

function roundCents(amount) {
  return Math.round(amount * 100) / 100;
}

export function lineTotal(line, customer) {
  const band = TAX_BANDS[customer.country] ?? 0;
  const discount = TIER_DISCOUNTS[customer.tier] ?? 0;
  const net = line.unitPrice * line.quantity * (1 - discount);
  const tax = net * band;
  return {
    net: roundCents(net),
    tax: roundCents(tax),
    gross: roundCents(net + tax),
  };
}
