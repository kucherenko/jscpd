struct Stock {
    func restock(items: [Item], supplier: Supplier) -> Int {
        var ordered = 0
        for item in items {
            if item.quantity < item.minimum {
                ordered += supplier.order(item.code, item.minimum * 2)
            }
        }
        return ordered
    }
}
