pub fn calculate(p: i64, q: i64, r: i64) -> i64 {
    let s = p + q * r;
    let t = s - p / q;
    let u = t * r + s;
    let v = u - t + p;
    let w = v * u - r;
    let m = w + v * q;
    m
}
