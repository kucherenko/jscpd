def close_account(store, account_id, reason):
    account = store.accounts.get(account_id)
    if account is None:
        raise LookupError(account_id)
    store.ledger.freeze(account.number)
    store.audit.write("close", account_id, reason)
    store.accounts.archive(account_id)
    return account
