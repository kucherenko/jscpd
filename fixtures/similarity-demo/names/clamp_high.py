def clamp_high(values, ceiling):
    result = []
    for value in values:
        if ceiling < value:
            value = ceiling
        result.append(value)
    return result
