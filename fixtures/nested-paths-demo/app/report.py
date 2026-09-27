"""A monthly summary of the ledger, one line per category."""

from lib.money import format_cents


def monthly_summary(entries: list[dict], month: str) -> list[str]:
    totals: dict[str, int] = {}
    for entry in entries:
        if not entry["date"].startswith(month):
            continue
        category = entry.get("category") or "other"
        totals[category] = totals.get(category, 0) + entry["cents"]
    lines = [f"Summary for {month}"]
    for category in sorted(totals, key=lambda c: -totals[c]):
        lines.append(f"  {category:<12} {format_cents(totals[category])}")
    lines.append(f"  {'total':<12} {format_cents(sum(totals.values()))}")
    return lines
