// Sales tax by state, in cents, rounded the way the payment provider rounds.

const RATES: Record<string, number> = { CA: 0.0725, NY: 0.04, TX: 0.0625, WA: 0.065 };

export function salesTax(amountCents: number, state: string, exempt: boolean): number {
  if (exempt || amountCents <= 0) {
    return 0;
  }
  const rate = RATES[state] ?? 0;
  const tax = amountCents * rate;
  const rounded = Math.round(tax);
  return rounded < 1 ? 1 : rounded;
}
