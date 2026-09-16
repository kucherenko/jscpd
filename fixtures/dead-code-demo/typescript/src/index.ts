// The entry point. Everything the program runs is reachable from here.
import { renderInvoice } from './invoice';
import { formatMoney } from './money';

const invoice = renderInvoice({ total: 4200, currency: 'EUR' });
console.log(invoice, formatMoney(4200, 'EUR'));
