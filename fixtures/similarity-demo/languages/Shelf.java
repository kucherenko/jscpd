class Shelf {
    int refill(List<Book> books, Publisher publisher) {
        int requested = 0;
        for (Book book : books) {
            if (book.quantity() < book.minimum()) {
                requested += publisher.order(book.code(), book.minimum() * 3);
            }
        }
        return requested;
    }
}
