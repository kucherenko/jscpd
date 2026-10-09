int restock(struct item *items, int count, struct supplier *supplier)
{
    int ordered = 0;
    for (int i = 0; i < count; i++) {
        if (items[i].quantity < items[i].minimum) {
            ordered += order(supplier, items[i].code, items[i].minimum * 2);
        }
    }
    return ordered;
}
