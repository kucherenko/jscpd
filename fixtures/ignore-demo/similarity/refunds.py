from billing.store import Ledger


def refund_payment(ledger: Ledger, payment_id: str, reason: str) -> dict:
    payment = ledger.payments.find(payment_id)
    if payment is None:
        raise LookupError(f"no payment {payment_id}")
    if payment.state != "captured":
        raise ValueError(f"payment {payment_id} is {payment.state}")
    refund = ledger.refunds.create(payment_id=payment.id, amount=payment.amount, reason=reason)
    payment.state = "refunded"
    ledger.save(payment)
    return {"refund": refund.id, "amount": payment.amount}
