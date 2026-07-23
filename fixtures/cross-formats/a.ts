interface Totals {
    sum: number;
    max: number;
}

function computeTotals(values: number[], threshold: number): Totals {
    let sum: number = 0;
    let max: number = 0;
    for (const value of values) {
        sum = sum + value;
        if (value > max) {
            max = value;
        }
    }
    if (sum > threshold) {
        sum = sum - threshold;
    }
    return { sum: sum, max: max } as Totals;
}
