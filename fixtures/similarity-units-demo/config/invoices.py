from billing import invoices, journal, staff
from billing.http import Router, Timeout

ROUTES = Router(
    prefix="/invoices",
    middleware=[staff.require_admin(), journal.record_calls(level="debug")],
    handlers={
        "list": invoices.list_invoices,
        "issue": invoices.issue_invoice,
        "void": invoices.void_invoice,
    },
    timeout=Timeout(connect=5.0, read=30.0),
)
