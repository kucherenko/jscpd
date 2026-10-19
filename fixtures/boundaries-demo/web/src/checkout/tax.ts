// The tax line of the order summary: the backend's rule, renamed and with
// a debug line added.

const STATE_RATES: Record<string, number> = { CA: 0.0725, NY: 0.04, TX: 0.0625, WA: 0.065 };

export function estimateTax(cents: number, region: string, taxFree: boolean): number {
  if (taxFree || cents <= 0) {
    return 0;
  }
  const rate = STATE_RATES[region] ?? 0;
  console.debug('tax rate', region, rate);
  const tax = cents * rate;
  const rounded = Math.round(tax);
  return rounded < 1 ? 1 : rounded;
}
