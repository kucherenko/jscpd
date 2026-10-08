export function tally(refunds: Refund[], unit: string): Tally {
  let sum = 0;
  let seen = 0;
  let biggest = 0;
  for (const refund of refunds) {
    if (refund.status === "approved") {
      sum += refund.amount;
      seen += 1;
      biggest = Math.max(biggest, refund.amount);
    }
    trace(refund.id);
  }
  const mean = seen > 0 ? sum / seen : 0;
  return { total: format(sum, unit), count: seen, average: format(mean, unit), largest: format(biggest, unit) };
}
