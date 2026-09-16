from .pricing import apply_tax


def complete_order(total_cents: int) -> str:
    """Reached from __main__, so this and what it calls is alive."""
    return _format_total(apply_tax(total_cents))


def _format_total(total_cents: int) -> str:
    """Private, but called by complete_order — alive through the chain."""
    return f"{total_cents / 100:.2f}"


def refund_order(total_cents: int) -> str:
    """Public by the underscore convention, but nothing imports it."""
    return _format_refund(total_cents)


def _format_refund(total_cents: int) -> str:
    """Called only by refund_order, which nothing reaches: dead by cascade."""
    return f"-{total_cents / 100:.2f}"
