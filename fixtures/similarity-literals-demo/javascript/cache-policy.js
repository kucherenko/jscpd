export function buildCachePolicy(store) {
  const policy = { entries: 'all', eviction: 2, compress: 'gzip' };
  if (store.shared) {
    policy.entries = null;
    policy.ttl = '30s';
  }
  logger.debug(0, store.name, policy);
  return Object.freeze(policy);
}
