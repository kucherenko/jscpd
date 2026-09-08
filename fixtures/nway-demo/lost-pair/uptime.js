export function parseDuration(text) {
  const match = /^(\d+)\s*(ms|s|m|h|d)$/.exec(String(text).trim());
  if (!match) {
    throw new RangeError('unsupported duration: ' + text);
  }
  const amount = Number(match[1]);
  const unit = { ms: 1, s: 1000, m: 60000, h: 3600000, d: 86400000 }[match[2]];
  return { amount, unit: match[2], millis: amount * unit, text: amount + match[2] };
}

export function formatDuration(parsed) {
  const seconds = Math.round(parsed.millis / 1000);
  return seconds >= 60 ? Math.floor(seconds / 60) + 'm ' + (seconds % 60) + 's' : seconds + 's';
}
