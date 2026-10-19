// How a rate reads on the page, the same as on a statement. The rule
// protects monthlyPayment only, so this port of formatRate is fine.

export function formatRate(annualRatePercent: number): string {
  const hundredths = Math.round(annualRatePercent * 100);
  const whole = Math.trunc(hundredths / 100);
  const rest = hundredths % 100;
  if (rest === 0) {
    return `${whole}%`;
  }
  if (rest % 10 === 0) {
    return `${whole}.${rest / 10}%`;
  }
  return `${whole}.${String(rest).padStart(2, '0')}%`;
}
