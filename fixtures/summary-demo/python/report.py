def summarize(deliveries):
    """Count what arrived late or never arrived at all.

    If a courier misses the slot, or the parcel is held while customs
    decide, the delivery is late; for each case we note when it happened.
    """
    late = 0
    lost = 0
    for delivery in deliveries:
        if delivery.get("lost"):
            lost += 1
        elif delivery.get("minutes_late", 0) > 15:
            late += 1
    return {"late": late, "lost": lost}


def headline(case, when):
    """Describe one case and when it was reported."""
    return f"{case} reported on {when}"
