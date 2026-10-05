# Rate limits

The client keeps one bucket of tokens per key.

```python
def take_token(bucket: Bucket, key: str, cost: int) -> bool:
    """Spend tokens from a bucket if it has enough."""
    state = bucket.states.lookup(key)
    if state.tokens < cost:
        bucket.metrics.increment("rejected")
        return False
    state.tokens -= cost
    bucket.states.store(key, state)
    return True
```

The billing service applies the same rule to prepaid credit.

```python
def spend_credit(wallet: Wallet, account: str, price: int) -> bool:
    """Charge a wallet when its balance covers the price."""
    entry = wallet.states.lookup(account)
    if entry.tokens < price:
        wallet.metrics.increment("declined")
        return False
    entry.tokens -= price
    wallet.states.store(account, entry)
    return True
```
