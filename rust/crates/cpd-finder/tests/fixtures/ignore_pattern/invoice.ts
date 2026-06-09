function calculateTotal(base) {
  const tax = base * 0.1;
  const shipping = 5.0;
  return base + tax + shipping;
}

function formatOutput(label, value) {
  const formatted = label + ": " + value.toFixed(2);
  return formatted;
}