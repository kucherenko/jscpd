/** Imported by index.ts and invoice.ts. */
export function formatMoney(amount: number, currency: string): string {
  return `${(amount / 100).toFixed(2)} ${currency}`;
}

/** Imported by invoice.ts but never called there: an unused import. */
export function roundToCents(amount: number): number {
  return Math.round(amount * 100) / 100;
}
