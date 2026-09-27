"""Amounts are kept in cents so that sums never drift."""

from decimal import Decimal, ROUND_HALF_EVEN


def to_cents(amount: str) -> int:
    value = Decimal(amount).quantize(Decimal("0.01"), rounding=ROUND_HALF_EVEN)
    return int(value * 100)


def format_cents(cents: int, currency: str = "EUR") -> str:
    sign = "-" if cents < 0 else ""
    whole, part = divmod(abs(cents), 100)
    grouped = f"{whole:,}".replace(",", " ")
    return f"{sign}{grouped}.{part:02d} {currency}"


def split_evenly(cents: int, parts: int) -> list[int]:
    if parts <= 0:
        raise ValueError("parts must be positive")
    base, rest = divmod(cents, parts)
    return [base + 1 if i < rest else base for i in range(parts)]
