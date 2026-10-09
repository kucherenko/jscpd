// Holds: where a patron stands in the queue for an item, and when a hold
// waiting on the pickup shelf lapses.

function daysLate(dueDate, returnedAt) {
  const due = new Date(dueDate);
  const returned = new Date(returnedAt);
  due.setHours(0, 0, 0, 0);
  returned.setHours(0, 0, 0, 0);
  const days = Math.round((returned - due) / 86400000);
  return days > 0 ? days : 0;
}

export function holdsAhead(hold, queue) {
  if (!hold || hold.cancelledAt) {
    return 0;
  }
  const waiting = queue.entries.filter((e) => e.active).length;
  const position = queue.positions[hold.patronId] ?? queue.defaultPosition;
  return Math.max(waiting - position, 0);
}

export function pickupExpired(hold, today) {
  return hold.shelvedAt !== undefined && daysLate(hold.pickupBy, today) > 0;
}
