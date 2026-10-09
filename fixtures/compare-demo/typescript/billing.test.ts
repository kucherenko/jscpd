// Tests of the invoice arithmetic, ported from python/test_billing.py.
import { describe, expect, it } from 'vitest';
import { applyDiscount, lineTotal } from './billing';

describe('billing', () => {
  it('line total sums quantity times price', () => {
    const lines = [{ quantity: 2, unitPrice: 150 }, { quantity: 1, unitPrice: 400 }];
    const total = lineTotal(lines);
    expect(total).toBe(700);
    expect(lineTotal([])).toBe(0);
  });

  it('line total rejects a zero quantity', () => {
    const lines = [{ quantity: 0, unitPrice: 150 }];
    expect(() => lineTotal(lines)).toThrow('quantity must be positive');
    expect(() => lineTotal([{ quantity: -3, unitPrice: 10 }])).toThrow(RangeError);
  });

  it('discount is capped', () => {
    expect(applyDiscount(10000, 10, 5000)).toBe(9000);
    expect(applyDiscount(10000, 90, 2500)).toBe(7500);
    expect(() => applyDiscount(10000, 120, 100)).toThrow(RangeError);
  });
});
