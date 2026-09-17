export function formatWeight(grams: number): string {
  return `${(grams / 1000).toFixed(2)} kg`;
}

export function convertToPounds(grams: number): number {
  return grams / 453.59237;
}

export function describeParcel(grams: number): string {
  return grams > 1000 ? 'heavy' : 'light';
}
