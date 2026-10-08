package library

func Refill(books []Book, publisher Publisher) int {
	requested := 0
	for _, book := range books {
		if book.Quantity < book.Minimum {
			requested += publisher.Order(book.Code, book.Minimum*3)
		}
	}
	return requested
}
