// Labels for the circulation desk screen.

export function formatDue(date) {
  return new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short" }).format(date);
}

export function formatFine(cents) {
  return `£${(cents / 100).toFixed(2)}`;
}

export function padLeft(text, width) {
  return String(text).padStart(width, " ");
}
