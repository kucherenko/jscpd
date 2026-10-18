def limit_low(items, minimum):
    kept = []
    for item in items:
        if item < minimum:
            item = minimum
        kept.append(item)
    return kept
