const pending = [];

export function enqueue(sku, copies) {
  pending.push({ sku, copies, queuedAt: Date.now() });
  return pending.length;
}

// Written for a "flush on shutdown" hook that never shipped.
export function drainQueue() {
  const drained = pending.splice(0, pending.length);
  return drained.reduce((total, label) => total + label.copies, 0);
}
