package warehouse

func Restock(items []Item, supplier Supplier) int {
	ordered := 0
	for _, item := range items {
		if item.Quantity < item.Minimum {
			ordered += supplier.Order(item.Code, item.Minimum*2)
		}
	}
	return ordered
}
