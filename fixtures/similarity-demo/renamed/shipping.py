def shipping_cost(parcel, zone, rates):
    weight = max(parcel.weight, 0.5)
    base = rates.lookup(zone, "standard")
    surcharge = 0
    if parcel.fragile:
        surcharge = base * 0.15
    total = round(base * weight + surcharge, 2)
    return min(total, 250.0)
