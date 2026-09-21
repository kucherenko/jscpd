import { enqueue } from '../src/print-queue.js';

// The job runner reads this to know when; no file in the project does.
export const schedule = '15 2 * * *';

export default async function reprintSmudged(context) {
  const smudged = await context.store.list({ status: 'smudged' });
  for (const label of smudged) {
    enqueue(label.sku, 1);
  }
  return smudged.length;
}
