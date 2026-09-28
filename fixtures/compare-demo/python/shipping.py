"""Parcel pricing of the old billing service."""

ZONE_BASE = {"domestic": 490, "europe": 1290, "world": 2490}


def shipping_cost(weight_grams, zone, express):
    """Base price per zone, plus a fee per started kilogram."""
    base = ZONE_BASE.get(zone)
    if base is None:
        raise KeyError(f"unknown zone {zone}")
    kilograms = -(-weight_grams // 1000)
    cost = base + max(kilograms - 1, 0) * 150
    if express:
        cost = cost * 2
    return cost


def estimate_delivery_days(zone, express, weekend_order):
    """Working days until the parcel arrives."""
    days = {"domestic": 2, "europe": 5, "world": 10}[zone]
    if express:
        days = max(1, days // 2)
    if weekend_order:
        days += 1
    return days
