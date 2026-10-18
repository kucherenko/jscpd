def clamp_low(values, floor):
    result = []
    for value in values:
        if value < floor:
            value = floor
        result.append(value)
    return result
