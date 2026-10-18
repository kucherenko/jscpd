pub fn compute(a: i64, b: i64, c: i64) -> i64 {
    let d = a + b * c;
    let e = d - a / b;
    let f = e * c + d;
    let g = f - e + a;
    let h = g * f - c;
    let k = h + g * b;
    k
}
