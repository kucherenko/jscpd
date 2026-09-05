export function basketTotal(entries, vatRate) {
  let net = 0;
  for (const entry of entries) {
    net += entry.price * entry.quantity;
  }
  const vat = net * vatRate;
  const gross = net + vat;
  return { net, vat, gross };
}
