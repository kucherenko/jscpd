"""A package whose __init__ is its public surface, so its imports are exports."""

from .checkout import complete_order

__all__ = ["complete_order"]
