// Loans: how late an item is, how many renewals are left, and what the
// circulation desk should do with it.

function daysLate(dueDate, returnedAt) {
  const due = new Date(dueDate);
  const returned = new Date(returnedAt);
  due.setHours(0, 0, 0, 0);
  returned.setHours(0, 0, 0, 0);
  const days = Math.round((returned - due) / 86400000);
  return days > 0 ? days : 0;
}

export function renewalsLeft(loan, policy) {
  if (!loan || loan.returnedAt) {
    return 0;
  }
  const used = loan.renewals.filter((r) => r.approved).length;
  const limit = policy.maxRenewals[loan.itemType] ?? policy.defaultRenewals;
  return Math.max(limit - used, 0);
}

export function loanStatus(loan, patron, today) {
  if (!loan) return "unknown";
  if (loan.lost || loan.damaged) return "closed";
  if (loan.returnedAt && daysLate(loan.dueDate, loan.returnedAt) > 0) return "returned-late";
  if (loan.returnedAt) return "returned";
  if (patron.suspended && !patron.appealPending) return "blocked";
  const late = daysLate(loan.dueDate, today);
  if (late > 30 || (late > 14 && patron.warnings > 2)) return "billed";
  if (late > 14) return "second-notice";
  if (late > 0 && loan.renewals.length < 3) return "renewable";
  if (late > 0) return "overdue";
  for (const hold of loan.holds) {
    if (hold.active && hold.patronId !== patron.id) return "recall";
  }
  return loan.autoRenew ? "auto" : "open";
}
