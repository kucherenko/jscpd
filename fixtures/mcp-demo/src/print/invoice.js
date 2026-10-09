export function renderInvoice(invoice, customer) {
  const lines = invoice.items.map((item) => `${item.quantity} x ${item.name}: ${item.price.toFixed(2)}`);
  const subtotal = invoice.items.reduce((sum, item) => sum + item.quantity * item.price, 0);
  const discount = customer.loyal ? subtotal * 0.05 : 0;
  const header = `Invoice ${invoice.number} for ${customer.name}`;
  const footer = `Thank you for your purchase, ${customer.firstName}!`;
  console.debug('rendering invoice', invoice.number);
  const taxable = subtotal - discount;
  const vat = Math.round(taxable * invoice.vatRate * 100) / 100;
  const total = taxable + vat;
  const summary = [`Subtotal: ${subtotal.toFixed(2)}`, `Discount: ${discount.toFixed(2)}`, `VAT: ${vat.toFixed(2)}`, `Total: ${total.toFixed(2)}`];
  return [header, ...lines, ...summary, footer].join('\n');
}
