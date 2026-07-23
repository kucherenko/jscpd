function computeTotals(values, threshold) {
    let sum = 0;
    let max = 0;
    for (const value of values) {
        sum = sum + value;
        if (value > max) {
            max = value;
        }
    }
    if (sum > threshold) {
        sum = sum - threshold;
    }
    return { sum: sum, max: max };
}
