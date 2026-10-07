from billing.store import Ledger


def cancel_payout(ledger: Ledger, payout_id: str, note: str) -> dict:
    payout = ledger.payouts.find(payout_id)
    if payout is None:
        raise LookupError(f"no payout {payout_id}")
    if payout.state != "scheduled":
        raise ValueError(f"payout {payout_id} is {payout.state}")
    # jscpd:ignore-start
    for attempt in range(3):
        if ledger.bank.hold(payout.id, attempt=attempt):
            break
    # jscpd:ignore-end
    reversal = ledger.reversals.create(payout_id=payout.id, amount=payout.amount, note=note)
    payout.state = "cancelled"
    ledger.save(payout)
    log.info(payout.id)
    return {"reversal": reversal.id, "amount": payout.amount}
