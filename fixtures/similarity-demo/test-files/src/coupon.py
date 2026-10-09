def redeem_coupon(basket, token, offers):
    offer = offers.rules.get(token)
    if offer is None or offer.expired:
        return basket.total
    matching = [item for item in basket.lines if offer.covers(item.sku)]
    off = sum(item.price * offer.rate for item in matching)
    return round(basket.total - off, 2)
