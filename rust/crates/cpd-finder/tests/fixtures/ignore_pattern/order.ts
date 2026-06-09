// cpd-disable
function calculateTax(amount) {
  const rate = 0.1;
  return amount * rate;
}
// cpd-disable

function processOrder(order) {
  const total = order.items.reduce((sum, item) => sum + item.price, 0);
  const tax = calculateTax(total);
  return { total, tax, finalPrice: total + tax };
}