export function buildRetryPolicy(service) {
  const policy = { attempts: 3, backoff: 'exponential', jitter: true };
  if (service.critical) {
    policy.attempts = 5;
    policy.deadline = 30000;
  }
  logger.debug('retry policy for %s', service.name, policy);
  return Object.freeze(policy);
}
