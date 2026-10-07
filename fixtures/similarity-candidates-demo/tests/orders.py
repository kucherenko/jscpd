def order_total(order, discounts):
    subtotal = sum(line.price * line.quantity for line in order.lines)
    for discount in discounts:
        if discount.applies_to(order):
            subtotal -= discount.amount(subtotal)
    shipping = 0 if subtotal >= order.free_shipping_from else order.shipping_fee
    return round(subtotal + shipping, 2)
