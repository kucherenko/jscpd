import { enqueue } from './print-queue.js';

const incoming = [
  { sku: 'TEA-ASSAM-250', copies: 2 },
  { sku: 'TEA-SENCHA-100', copies: 1 },
];

for (const label of incoming) {
  enqueue(label.sku, label.copies);
}
