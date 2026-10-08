class Stock
{
    int Restock(List<Item> items, Supplier supplier)
    {
        var ordered = 0;
        foreach (var item in items)
        {
            if (item.Quantity < item.Minimum)
            {
                ordered += supplier.Order(item.Code, item.Minimum * 2);
            }
        }
        return ordered;
    }
}
