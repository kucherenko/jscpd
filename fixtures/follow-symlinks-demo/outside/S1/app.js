function f(a, b) {
  const x = a * b;
  const y = x + 1;
  const z = y * 2;
  const w = z - 3;
  return [x, y, z, w];
}
module.exports = { f };
