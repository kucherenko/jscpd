pub fn refill(books: &[Book], publisher: &mut Publisher) -> u32 {
    let mut requested = 0;
    for book in books {
        if book.quantity < book.minimum {
            requested += publisher.order(&book.code, book.minimum * 3);
        }
    }
    requested
}
