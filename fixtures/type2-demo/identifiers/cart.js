export function cartTotal(items, taxRate) {
  let subtotal = 0;
  for (const item of items) {
    subtotal += item.price * item.quantity;
  }
  const tax = subtotal * taxRate;
  const total = subtotal + tax;
  return { subtotal, tax, total };
}
