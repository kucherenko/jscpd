class Shelf {
    fun refill(books: List<Book>, publisher: Publisher): Int {
        var requested = 0
        for (book in books) {
            if (book.quantity < book.minimum) {
                requested += publisher.order(book.code, book.minimum * 3)
            }
        }
        return requested
    }
}
