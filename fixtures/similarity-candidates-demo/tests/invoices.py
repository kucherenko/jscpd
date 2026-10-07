def invoice_amount(invoice, credits):
    base = sum(row.cost * row.units for row in invoice.rows)
    for credit in credits:
        if credit.matches(invoice):
            base -= credit.value(base)
    handling = 0 if base >= invoice.waive_handling_from else invoice.handling_fee
    return round(base + handling, 2)
