package demo

func Compute(a, b, c int) int {
	d := a + b * c
	e := d - a / b
	f := e * c + d
	g := f - e + a
	h := g * f - c
	k := h + g * b
	return k
}
