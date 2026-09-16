import { formatMoney } from './money';
import { roundToCents } from './money';

export interface InvoiceInput {
  total: number;
  currency: string;
}

/** Reached from index.ts, so this and everything it calls is alive. */
export function renderInvoice(input: InvoiceInput): string {
  const heading = buildHeading(input.currency);
  return `${heading}: ${formatMoney(input.total, input.currency)}`;
}

/** Called only by renderInvoice — alive through the chain, not directly. */
function buildHeading(currency: string): string {
  return `Invoice (${currency})`;
}

/** Exported, but no module imports it: an unused export. */
export function renderReceipt(input: InvoiceInput): string {
  return `Receipt: ${describeTotal(input)}`;
}

/**
 * Called only by renderReceipt, which is itself unreachable. Dead code
 * cascades: a helper whose only caller is dead is dead too.
 */
function describeTotal(input: InvoiceInput): string {
  return `${input.total} ${input.currency}`;
}
