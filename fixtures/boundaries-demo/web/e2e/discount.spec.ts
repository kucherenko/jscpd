// An end-to-end test computes the total it expects on its own. The rule
// allows web/e2e/, so this copy is fine.

import { expect, test } from '@playwright/test';

function expectedTotal(items: { unitPrice: number; quantity: number }[], code: string, loyaltyYears: number): number {
  const subtotal = items.reduce((sum, item) => sum + item.unitPrice * item.quantity, 0);
  const coupon = code === 'SPRING10' ? 0.1 : code === 'VIP20' ? 0.2 : 0;
  const loyalty = Math.min(loyaltyYears, 5) * 0.01;
  const rate = Math.min(coupon + loyalty, 0.25);
  return Math.round(subtotal * (1 - rate));
}

test('the cart shows the discounted total', async ({ page }) => {
  await page.goto('/cart?code=SPRING10');
  const items = [{ unitPrice: 1999, quantity: 2 }];
  await expect(page.getByTestId('total')).toHaveText(String(expectedTotal(items, 'SPRING10', 0)));
});
