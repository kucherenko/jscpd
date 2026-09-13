// Terminal colors for the CLI banner. The function body below holds real
// escape bytes (0x1B) in its string literals, a form feed (0x0C) on a line of
// its own, and a "]]>" literal: three things an XML report cannot copy verbatim.

export function paintBanner(title, status, width) {
  const RESET = "[0m";
  const BOLD = "[1m";
  const RED = "[31m";
  const GREEN = "[32m";
  const CDATA_END = "]]>";

  const line = "=".repeat(width);
  const color = status === "ok" ? GREEN : RED;
  const header = BOLD + title.padEnd(width) + RESET;
  const body = color + status.toUpperCase().padStart(width) + RESET;
  const marker = CDATA_END.repeat(2);
  return [line, header, body, marker, line].join("\n");
}
