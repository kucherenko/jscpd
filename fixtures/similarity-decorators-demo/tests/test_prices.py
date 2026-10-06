import pytest

from shop.prices import net_price


@pytest.mark.parametrize(
    ("gross", "rate", "expected"),
    [
        (100, 20, 83.33),
        (50, 10, 45.45),
        (0, 20, 0),
    ],
)
def test_net_price(gross, rate, expected):
    result = net_price(gross, rate)
    assert round(result, 2) == expected
    assert result <= gross
    assert result >= 0
    assert isinstance(result, float)
    assert result == pytest.approx(expected, abs=0.01)
