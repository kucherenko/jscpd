// Nothing imports this file, so the file itself is the finding — basta does
// not also list the three declarations inside it.
import { formatMoney } from './money';

export function renderLegacyInvoice(total: number): string {
  return `Legacy: ${formatMoney(total, 'USD')}`;
}

export const LEGACY_TEMPLATE = 'legacy-invoice-v1';

export type LegacyOptions = { compact: boolean };
