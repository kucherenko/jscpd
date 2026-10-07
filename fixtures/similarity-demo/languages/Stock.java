class Stock {
    int restock(List<Item> items, Supplier supplier) {
        int ordered = 0;
        for (Item item : items) {
            if (item.quantity() < item.minimum()) {
                ordered += supplier.order(item.code(), item.minimum() * 2);
            }
        }
        return ordered;
    }
}
