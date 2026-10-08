pub fn restock(items: &[Item], supplier: &mut Supplier) -> u32 {
    let mut ordered = 0;
    for item in items {
        if item.quantity < item.minimum {
            ordered += supplier.order(&item.code, item.minimum * 2);
        }
    }
    ordered
}
