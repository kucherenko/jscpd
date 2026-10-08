object Shelf {
  def refill(books: Seq[Book], publisher: Publisher): Int = {
    var requested = 0
    for (book <- books) {
      if (book.quantity < book.minimum) {
        requested += publisher.order(book.code, book.minimum * 3)
      }
    }
    requested
  }
}
