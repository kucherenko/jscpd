package demo

func Calculate(p, q, r int) int {
	s := p + q * r
	t := s - p / q
	u := t * r + s
	v := u - t + p
	w := v * u - r
	m := w + v * q
	return m
}
