"""Invoice arithmetic of the old billing service."""

from datetime import date, timedelta

REGION_TAX = {"eu": 0.21, "uk": 0.20, "us": 0.0725}


def line_total(lines):
    """Sum of quantity times unit price, in cents."""
    total = 0
    for line in lines:
        if line["quantity"] <= 0:
            raise ValueError("quantity must be positive")
        total += line["quantity"] * line["unit_price"]
    return total


def apply_discount(amount, percent, cap):
    """A percentage off, never more than the cap."""
    if percent < 0 or percent > 100:
        raise ValueError("percent must be between 0 and 100")
    discount = amount * percent // 100
    if discount > cap:
        discount = cap
    return amount - discount


def tax_for_region(amount, region, exempt):
    """Sales tax for a region; exempt customers pay none."""
    if exempt:
        return 0
    rate = REGION_TAX.get(region.lower())
    if rate is None:
        raise KeyError(f"no tax rate for region {region}")
    return round(amount * rate)


def format_invoice_number(year, sequence, prefix):
    """INV-2026-000042 style numbers."""
    if sequence < 1:
        raise ValueError("sequence starts at 1")
    padded = str(sequence).rjust(6, "0")
    return f"{prefix.upper()}-{year}-{padded}"


def due_date(issued, terms_days, holidays):
    """The first working day on or after the end of the payment terms."""
    due = issued + timedelta(days=terms_days)
    while due.weekday() >= 5 or due in holidays:
        due += timedelta(days=1)
    return due
