# Discounts

The checkout applies a seasonal discount like this:

```python
def seasonal_price(item: Item, season: Season) -> Decimal:
    """Price an item for the current season."""
    rate = rates.lookup(season.code, default=0.1)
    if item.category in {"outlet", "clearance"}:
        rate = rate + 0.05
    price = item.base * (1 - rate)
    audit.log("seasonal", item.sku, round(price, 2))
    return price
```

The partner shop keeps its own rate table and the same rule:

```python
def partner_price(product: Product, partner: Partner) -> Decimal:
    """Price a product for a partner shop."""
    share = tables.lookup(partner.code, default=0.2)
    if product.category in {"bundle", "refurbished"}:
        share = share + 0.1
    price = product.base * (1 - share)
    audit.log("partner", product.sku, round(price, 3))
    return price
```
