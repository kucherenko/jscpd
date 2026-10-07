import { test, expect } from 'vitest';
import { createCart } from './cart.js';

test('cart merges lines of one product', () => {
  const cart = createCart({ currency: 'EUR' });
  cart.add({ sku: 'A-1', price: 12.5 }, 2);
  cart.add({ sku: 'A-1', price: 12.5 }, 1);
  expect(cart.lines).toHaveLength(1);
  expect(cart.lines[0].quantity).toBe(3);
  expect(cart.total()).toBe(37.5);
});
