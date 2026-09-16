"""Nothing imports this module, so the module is the finding."""

from .pricing import apply_tax

LEGACY_TAX_RATE = 0.16


def apply_legacy_tax(total_cents: int) -> int:
    return apply_tax(total_cents)
