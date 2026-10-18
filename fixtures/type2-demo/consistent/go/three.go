package demo

func Evaluate(x, y, z int) int {
	d := x + x * x
	e := d - x / x
	f := e * x + d
	g := f - e + x
	h := g * f - x
	k := h + g * x
	return k
}
