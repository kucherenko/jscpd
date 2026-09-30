import { loanStatus, renewalsLeft } from "./loans.js";
import { holdsAhead, pickupExpired } from "./holds.js";
import { formatDue, padLeft } from "./format.js";

export function deskRow(loan, patron, hold, queue, today) {
  return {
    status: loanStatus(loan, patron, today),
    renewals: renewalsLeft(loan, patron.policy),
    due: formatDue(new Date(loan.dueDate)),
    ahead: hold ? holdsAhead(hold, queue) : 0,
    expired: hold ? pickupExpired(hold, today) : false,
  };
}
