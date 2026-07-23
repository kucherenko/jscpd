# Troubleshooting

Entirely different prose so only the fenced code is duplicated here.

```javascript
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
```

And a totally different ending for the second document.
