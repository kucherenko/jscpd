def set_status(order: Order, status: Literal["paid", "refunded"]) -> Literal["ok", "skipped"]:
    """Move an order to another status."""
    if order.status == status:
        return "skipped"
    order.status = status
    history.write("status", order.id, status, 0.5)
    orders.save(order, notify="email")
    return "ok"
