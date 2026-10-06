from enum import IntEnum


class HttpStatus(IntEnum):
    """Status codes the gateway answers with."""

    OK = 200
    CREATED = 201
    ACCEPTED = 202
    NO_CONTENT = 204
    MOVED = 301
    NOT_MODIFIED = 304
    BAD_REQUEST = 400
    UNAUTHORIZED = 401
    FORBIDDEN = 403
    NOT_FOUND = 404
    CONFLICT = 409
    UNAVAILABLE = 503
    TOO_MANY = 429
    GONE = 410
    TIMEOUT = 504
    BAD_GATEWAY = 502


GATEWAY_ROUTES = [
    route("/health", handlers.health, method="GET"),
    route("/orders", handlers.create_order, method="POST"),
    route("/orders/<id>", handlers.order, method="GET"),
    route("/refunds", handlers.refund, method="POST"),
    route("/metrics", handlers.metrics, method="GET"),
]
