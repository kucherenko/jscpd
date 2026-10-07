from invoices import invoice_amount


def test_invoice_amount_applies_credits():
    invoice = make_invoice(rows=[(8.0, 4), (2.5, 6)], handling_fee=1.5)
    credits = [store_credit(5), refund_credit(3)]
    amount = invoice_amount(invoice, credits)
    assert amount == round((32.0 + 15.0) - 5 - 3 + 1.5, 2)
    assert invoice.rows[1].units == 6
    assert not invoice.settled
