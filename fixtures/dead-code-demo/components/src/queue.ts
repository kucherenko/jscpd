export function describeQueue(): string[] {
  return ['inbound', 'outbound'];
}

export function clampLevel(level: number): number {
  return Math.min(Math.max(level, 0), 10);
}
