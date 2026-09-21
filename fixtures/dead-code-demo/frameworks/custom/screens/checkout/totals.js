export function formatPence(pence) {
  return `£${(pence / 100).toFixed(2)}`;
}

export function splitBetween(pence, guests) {
  return Math.ceil(pence / Math.max(guests, 1));
}
