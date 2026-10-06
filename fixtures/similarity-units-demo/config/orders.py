from shop import audit, auth, orders
from shop.http import Router, Timeout

ROUTES = Router(
    prefix="/orders",
    middleware=[auth.require_user(), audit.log_requests(level="info")],
    handlers={
        "list": orders.list_orders,
        "create": orders.create_order,
        "cancel": orders.cancel_order,
    },
    timeout=Timeout(connect=2.0, read=10.0),
)
