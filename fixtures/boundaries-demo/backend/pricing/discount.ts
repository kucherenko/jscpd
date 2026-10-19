// The one place that turns a cart into the amount the customer pays.

export interface LineItem {
  sku: string;
  unitPrice: number;
  quantity: number;
}

export function applyDiscount(items: LineItem[], code: string, loyaltyYears: number): number {
  const subtotal = items.reduce((sum, item) => sum + item.unitPrice * item.quantity, 0);
  const coupon = code === 'SPRING10' ? 0.1 : code === 'VIP20' ? 0.2 : 0;
  const loyalty = Math.min(loyaltyYears, 5) * 0.01;
  const rate = Math.min(coupon + loyalty, 0.25);
  return Math.round(subtotal * (1 - rate));
}

export function isKnownCoupon(code: string): boolean {
  return ['SPRING10', 'VIP20'].includes(code.trim().toUpperCase());
}
