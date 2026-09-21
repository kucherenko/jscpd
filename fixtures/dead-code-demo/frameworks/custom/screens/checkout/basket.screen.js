import { formatPence } from './totals.js';

export default {
  mount(route) {
    const lines = [{ label: 'Flat white', pence: 340 }, { label: 'Almond croissant', pence: 295 }];
    const total = lines.reduce((sum, line) => sum + line.pence, 0);
    return { route, heading: 'Your basket', total: formatPence(total) };
  },
};
