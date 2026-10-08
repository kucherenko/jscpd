int refill(struct book *books, int total, struct publisher *publisher)
{
    int requested = 0;
    for (int j = 0; j < total; j++) {
        if (books[j].quantity < books[j].minimum) {
            requested += order(publisher, books[j].code, books[j].minimum * 3);
        }
    }
    return requested;
}
