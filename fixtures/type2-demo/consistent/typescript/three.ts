export function evaluate(x: number, y: number, z: number): number {
  const d = x + x * x;
  const e = d - x / x;
  const f = e * x + d;
  const g = f - e + x;
  const h = g * f - x;
  const k = h + g * x;
  return k;
}
