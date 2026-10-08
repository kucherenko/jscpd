int Shelf::refill(const std::vector<Book>& books, Publisher& publisher)
{
    int requested = 0;
    for (const auto& book : books) {
        if (book.quantity < book.minimum) {
            requested += publisher.order(book.code, book.minimum * 3);
        }
    }
    return requested;
}
