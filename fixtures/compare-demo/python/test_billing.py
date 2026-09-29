"""Tests of the invoice arithmetic."""

import pytest

from billing import apply_discount, due_date, line_total


def test_line_total_sums_quantity_times_price():
    lines = [{"quantity": 2, "unit_price": 150}, {"quantity": 1, "unit_price": 400}]
    total = line_total(lines)
    assert total == 700
    assert line_total([]) == 0


def test_line_total_rejects_a_zero_quantity():
    lines = [{"quantity": 0, "unit_price": 150}]
    with pytest.raises(ValueError, match="quantity must be positive"):
        line_total(lines)
    with pytest.raises(ValueError):
        line_total([{"quantity": -3, "unit_price": 10}])


def test_discount_is_capped():
    assert apply_discount(10000, 10, 5000) == 9000
    assert apply_discount(10000, 90, 2500) == 7500
    with pytest.raises(ValueError):
        apply_discount(10000, 120, 100)


def test_due_date_skips_the_weekend():
    from datetime import date

    issued = date(2026, 9, 1)
    due = due_date(issued, 4, set())
    assert due.weekday() < 5
    assert due >= date(2026, 9, 5)
