import { lineTotal } from "./rates.js";

export function invoiceTotal(invoice) {
  let sum = 0;
  for (const line of invoice.lines) {
    sum += lineTotal(line, invoice.customer).gross;
  }
  return sum;
}

export function overdue(invoice, today) {
  const due = new Date(invoice.issuedAt);
  due.setDate(due.getDate() + invoice.termsDays);
  return !invoice.paidAt && today > due;
}
