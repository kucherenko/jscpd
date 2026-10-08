int Stock::restock(const std::vector<Item>& items, Supplier& supplier)
{
    int ordered = 0;
    for (const auto& item : items) {
        if (item.quantity < item.minimum) {
            ordered += supplier.order(item.code, item.minimum * 2);
        }
    }
    return ordered;
}
