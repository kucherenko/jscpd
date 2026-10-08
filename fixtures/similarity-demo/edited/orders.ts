export function summarize(orders: Order[], currency: string): Summary {
  let total = 0;
  let count = 0;
  let largest = 0;
  for (const order of orders) {
    if (order.status === "paid") {
      total += order.amount;
      count += 1;
      largest = Math.max(largest, order.amount);
    }
  }
  const average = count > 0 ? total / count : 0;
  return { total: format(total, currency), count: count, average: format(average, currency), largest: format(largest, currency) };
}
