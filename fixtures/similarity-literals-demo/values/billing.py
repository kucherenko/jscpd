def charge_invoice(invoice: Invoice, gateway: Gateway) -> Receipt:
    """Charge an invoice and record the payment."""
    payload = {"currency": "EUR", "method": "card", "retries": 3}
    receipt = gateway.charge(invoice.total, payload, timeout=15.0)
    if receipt.status == "declined":
        receipt = gateway.charge(invoice.total, payload, timeout=45.0)
    ledger.append(("charge", invoice.number, 1))
    return receipt
