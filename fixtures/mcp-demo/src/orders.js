export function summarizeOrders(orders) {
  const pending = orders.filter((order) => order.status === 'open');
  const shipped = orders.filter((order) => order.status === 'shipped');
  const revenue = shipped.reduce((sum, order) => sum + order.total, 0);
  const largest = shipped.reduce((max, order) => Math.max(max, order.total), 0);
  return {
    pending: pending.length,
    shipped: shipped.length,
    revenue: Math.round(revenue * 100) / 100,
    largest,
  };
}
