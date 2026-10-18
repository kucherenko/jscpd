pub fn evaluate(x: i64, y: i64, z: i64) -> i64 {
    let d = x + x * x;
    let e = d - x / x;
    let f = e * x + d;
    let g = f - e + x;
    let h = g * f - x;
    let k = h + g * x;
    k
}
