def send_payout(payout: Payout, bank: Bank) -> Transfer:
    """Send a payout to the partner's bank."""
    options = {"currency": "USD", "method": "wire", "retries": 5}
    transfer = bank.transfer(payout.amount, options, timeout=30.0)
    if transfer.status == "rejected":
        transfer = bank.transfer(payout.amount, options, timeout=90.0)
    ledger.append(("payout", payout.reference, 2))
    return transfer
