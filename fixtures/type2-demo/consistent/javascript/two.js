export function calculate(p, q, r) {
  const s = p + q * r;
  const t = s - p / q;
  const u = t * r + s;
  const v = u - t + p;
  const w = v * u - r;
  const m = w + v * q;
  return m;
}
