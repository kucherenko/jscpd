import { test, expect } from 'vitest';
import { createWishlist } from './wishlist.js';

test('wishlist keeps one entry per book', () => {
  const list = createWishlist({ owner: 'ana' });
  list.save({ isbn: '978-0', copies: 1 }, 2);
  list.save({ isbn: '978-0', copies: 1 }, 1);
  expect(list.entries).toHaveLength(1);
  expect(list.entries[0].count).toBe(3);
  expect(list.size()).toBe(1);
});
