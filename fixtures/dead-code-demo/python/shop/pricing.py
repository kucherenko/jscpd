import os
import math

TAX_RATE = 0.19


def apply_tax(total_cents: int) -> int:
    """Imported by checkout.py."""
    return int(total_cents * (1 + TAX_RATE))


def _unused_rounding(value: float) -> int:
    """Private and never called."""
    return math.floor(value)
