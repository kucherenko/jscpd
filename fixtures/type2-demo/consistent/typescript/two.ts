export function calculate(p: number, q: number, r: number): number {
  const s = p + q * r;
  const t = s - p / q;
  const u = t * r + s;
  const v = u - t + p;
  const w = v * u - r;
  const m = w + v * q;
  return m;
}
