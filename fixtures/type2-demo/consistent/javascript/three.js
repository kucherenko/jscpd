export function evaluate(x, y, z) {
  const d = x + x * x;
  const e = d - x / x;
  const f = e * x + d;
  const g = f - e + x;
  const h = g * f - x;
  const k = h + g * x;
  return k;
}
