// The loan calculator widget: the annuity formula of the backend, written
// again in TypeScript, with floating-point cents.

export function monthlyPayment(principal: number, yearlyRate: number, termMonths: number): number {
  if (termMonths <= 0) {
    return principal;
  }
  const r = yearlyRate / 100 / 12;
  if (r === 0) {
    return principal / termMonths;
  }
  const growth = Math.pow(1 + r, termMonths);
  return Math.round((principal * r * growth) / (growth - 1));
}
