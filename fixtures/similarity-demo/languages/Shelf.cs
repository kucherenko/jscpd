class Shelf
{
    int Refill(List<Book> books, Publisher publisher)
    {
        var requested = 0;
        foreach (var book in books)
        {
            if (book.Quantity < book.Minimum)
            {
                requested += publisher.Order(book.Code, book.Minimum * 3);
            }
        }
        return requested;
    }
}
