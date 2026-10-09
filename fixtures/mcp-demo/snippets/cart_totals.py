def cart_totals(lines, coupon):
    subtotal = sum(line.unit_price_cents * line.quantity for line in lines)
    if coupon is None:
        discount = 0
    elif coupon.kind == "percent":
        discount = subtotal * coupon.percent // 100
    else:
        discount = min(coupon.amount_cents, subtotal)
    taxable = subtotal - discount
    tax = taxable * 20 // 100
    shipping = 0 if taxable >= 5000 else 499
    return {"subtotal": subtotal, "discount": discount, "tax": tax, "shipping": shipping, "total": taxable + tax + shipping}
