"""Order totals for the storefront checkout."""


def line_total(quantity, unit_price, discount_rate):
    gross = quantity * unit_price
    discount = round(gross * discount_rate, 2)
    return max(gross - discount, 0)


def order_total(lines, coupon_rate, shipping_fee, free_from):
    subtotal = 0
    for quantity, unit_price, discount_rate in lines:
        subtotal += line_total(quantity, unit_price, discount_rate)
    subtotal -= round(subtotal * coupon_rate, 2)
    tax_rate = 0.20
    tax = round(subtotal * tax_rate, 2)
    shipping = 0 if subtotal >= free_from else shipping_fee
    return {
        "subtotal": subtotal,
        "tax": tax,
        "shipping": shipping,
        "total": subtotal + tax + shipping,
    }


def format_receipt(totals, currency):
    rows = []
    for key in ("subtotal", "tax", "shipping", "total"):
        rows.append(f"{key:>10}: {totals[key]:>10.2f} {currency}")
    return "\n".join(rows)
