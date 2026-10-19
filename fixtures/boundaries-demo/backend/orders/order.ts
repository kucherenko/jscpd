// Another backend module with its own copy of the discount: duplication,
// but not a leak, since the rule keeps copies out of web/ only.

import type { LineItem } from '../pricing/discount';

export function orderTotal(items: LineItem[], code: string, loyaltyYears: number): number {
  const subtotal = items.reduce((sum, item) => sum + item.unitPrice * item.quantity, 0);
  const coupon = code === 'SPRING10' ? 0.1 : code === 'VIP20' ? 0.2 : 0;
  const loyalty = Math.min(loyaltyYears, 5) * 0.01;
  const rate = Math.min(coupon + loyalty, 0.25);
  return Math.round(subtotal * (1 - rate));
}

export function orderNumber(sequence: number, placedAt: Date): string {
  const year = placedAt.getUTCFullYear();
  return `ORD-${year}-${String(sequence).padStart(6, '0')}`;
}
