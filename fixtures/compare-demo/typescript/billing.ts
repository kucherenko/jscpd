// Invoice arithmetic, ported from python/billing.py.

export interface Line {
  quantity: number;
  unitPrice: number;
}

const TAX_BY_REGION: Record<string, number> = { eu: 0.21, uk: 0.2, us: 0.0725 };

export function lineTotal(lines: Line[]): number {
  return lines.reduce((sum, line) => {
    if (line.quantity <= 0) {
      throw new RangeError('quantity must be positive');
    }
    return sum + line.quantity * line.unitPrice;
  }, 0);
}

export function applyDiscount(amount: number, percent: number, cap: number): number {
  if (percent < 0 || percent > 100) {
    throw new RangeError('percent must be between 0 and 100');
  }
  const discount = Math.min(Math.floor((amount * percent) / 100), cap);
  return amount - discount;
}

export function salesTax(amount: number, region: string, exempt: boolean): number {
  if (exempt) return 0;
  const rate = TAX_BY_REGION[region.toLowerCase()];
  if (rate === undefined) {
    throw new Error(`no tax rate for region ${region}`);
  }
  return Math.round(amount * rate);
}

export const formatInvoiceNumber = (year: number, sequence: number, prefix: string) =>
  `${prefix.toUpperCase()}-${year}-${String(sequence).padStart(6, '0')}`;

export function toCurrency(cents: number, currency: string, locale: string): string {
  const formatter = new Intl.NumberFormat(locale, {
    style: 'currency',
    currency,
    minimumFractionDigits: 2,
  });
  return formatter.format(cents / 100);
}
