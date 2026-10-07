from orders import order_total


def test_order_total_applies_discounts():
    order = make_order(lines=[(10.0, 3), (5.5, 2)], shipping_fee=4.99)
    discounts = [percent_off(10), fixed_off(2)]
    total = order_total(order, discounts)
    assert total == round((30.0 + 11.0) * 0.9 - 2 + 4.99, 2)
    assert order.lines[0].quantity == 3
    assert not order.paid
