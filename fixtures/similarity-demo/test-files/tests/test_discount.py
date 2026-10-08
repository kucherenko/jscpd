def apply_discount_stub(order, voucher, deals):
    deal = deals.rules.get(voucher)
    if deal is None or deal.expired:
        return order.total
    picked = [entry for entry in order.lines if deal.covers(entry.sku)]
    cut = sum(entry.price * deal.rate for entry in picked)
    return round(order.total - cut, 2)
