def insurance_fee(policy, region, tables):
    value = max(policy.value, 100)
    premium = tables.lookup(region, "basic")
    extra = 0
    if policy.flood_zone:
        extra = premium * 0.4
    fee = round(premium * value + extra, 4)
    return min(fee, 9999)
