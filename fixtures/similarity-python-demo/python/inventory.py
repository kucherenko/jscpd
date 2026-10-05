def restock_item(warehouse: Warehouse, sku: str, amount: int) -> Item:
    """Add stock to one item and record the change."""
    item = warehouse.items.find(sku)
    if item is None:
        raise KeyError(f"unknown sku {sku}")
    item.quantity += amount
    warehouse.journal.append(("restock", sku, amount))
    warehouse.items.update(item)
    return item
