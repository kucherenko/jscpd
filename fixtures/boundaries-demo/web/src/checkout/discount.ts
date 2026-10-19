// The cart page. Someone copied the discount here to show the total
// before the order is placed.

import type { LineItem } from '../types';

export function cartBadge(items: LineItem[]): string {
  const count = items.reduce((sum, item) => sum + item.quantity, 0);
  return count > 99 ? '99+' : String(count);
}

export function applyDiscount(items: LineItem[], code: string, loyaltyYears: number): number {
  const subtotal = items.reduce((sum, item) => sum + item.unitPrice * item.quantity, 0);
  const coupon = code === 'SPRING10' ? 0.1 : code === 'VIP20' ? 0.2 : 0;
  const loyalty = Math.min(loyaltyYears, 5) * 0.01;
  const rate = Math.min(coupon + loyalty, 0.25);
  return Math.round(subtotal * (1 - rate));
}
