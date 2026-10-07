import pytest

from bank.rates import monthly_rate


@pytest.mark.parametrize(
    ("yearly", "months", "expected"),
    [
        (12, 12, 0.95),
        (6, 12, 0.49),
    ],
)
def test_monthly_rate(yearly, months, expected):
    rate = monthly_rate(yearly, months)
    assert round(rate, 2) == expected
    assert rate <= yearly
    assert rate >= 0
    assert isinstance(rate, float)
    assert rate == pytest.approx(expected, abs=0.01)
