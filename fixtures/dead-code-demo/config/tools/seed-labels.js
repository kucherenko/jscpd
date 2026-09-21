import { enqueue } from '../src/print-queue.js';

// Run by hand on a fresh station: `node tools/seed-labels.js`.
const SAMPLES = ['TEA-ASSAM-250', 'TEA-SENCHA-100', 'TEA-ROOIBOS-80'];

for (const sku of SAMPLES) {
  enqueue(sku, 1);
}
console.log(`queued ${SAMPLES.length} sample labels`);
