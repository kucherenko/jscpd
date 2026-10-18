export function compute(a, b, c) {
  const d = a + b * c;
  const e = d - a / b;
  const f = e * c + d;
  const g = f - e + a;
  const h = g * f - c;
  const k = h + g * b;
  return k;
}
