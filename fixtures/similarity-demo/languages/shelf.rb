class Shelf
  def refill(books, publisher)
    requested = 0
    books.each do |book|
      if book.quantity < book.minimum
        requested += publisher.order(book.code, book.minimum * 3)
      end
    end
    requested
  end
end
