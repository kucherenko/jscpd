//! Fixtures shared by the integration test binaries of this crate. Each
//! binary compiles this module on its own, so unused items are expected.
#![allow(dead_code)]

/// A function and a structurally similar rewrite of it (other names, one
/// extra statement, one extra call): no exact clone, but `--similarity 0.7`
/// matches them. Used by the CLI and the MCP tests.
pub const INVOICE_JS: &str = "export function buildInvoice(order, customer, taxRate) {\n  const lines = [];\n  for (const item of order.items) {\n    const net = item.price * item.quantity;\n    lines.push({ sku: item.sku, quantity: item.quantity, net });\n  }\n  const subtotal = lines.reduce((sum, line) => sum + line.net, 0);\n  const tax = Math.round(subtotal * taxRate * 100) / 100;\n  return { number: nextInvoiceNumber(), customer: customer.id, lines, subtotal, tax, total: subtotal + tax };\n}\n";
pub const CREDIT_NOTE_JS: &str = "export function buildCreditNote(refund, account, vatRate) {\n  const entries = [];\n  for (const item of refund.items) {\n    if (!item.refundable) continue;\n    const net = item.price * item.quantity;\n    entries.push({ sku: item.sku, quantity: item.quantity, net });\n  }\n  const subtotal = entries.reduce((sum, entry) => sum + entry.net, 0);\n  const vat = Math.round(subtotal * vatRate * 100) / 100;\n  logger.info('credit note', { account: account.id, subtotal });\n  return { number: nextCreditNoteNumber(), account: account.id, entries, subtotal, vat, total: subtotal + vat };\n}\n";
