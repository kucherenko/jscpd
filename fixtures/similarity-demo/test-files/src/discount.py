def apply_discount(cart, code, catalog):
    rule = catalog.rules.get(code)
    if rule is None or rule.expired:
        return cart.total
    eligible = [line for line in cart.lines if rule.covers(line.sku)]
    saved = sum(line.price * rule.rate for line in eligible)
    return round(cart.total - saved, 2)
