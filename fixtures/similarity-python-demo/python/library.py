def return_book(shelf: Shelf, isbn: str, copies: int) -> Book:
    """Put copies of a book back on the shelf and log it."""
    book = shelf.items.find(isbn)
    if book is None:
        raise KeyError(f"no such isbn {isbn}")
    book.quantity += copies
    shelf.journal.append(("return", isbn, copies))
    shelf.items.update(book)
    return book
