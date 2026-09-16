"""Started by `python -m shop`, so nothing needs to import it."""

from .checkout import complete_order

if __name__ == "__main__":
    print(complete_order(4200))
