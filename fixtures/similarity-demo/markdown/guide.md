# Retrying requests

A client retries a request a few times before it gives up, waiting longer
after every failed attempt:

```python
def fetch_with_retry(client, url, attempts):
    delay = 0.5
    for attempt in range(attempts):
        response = client.get(url, timeout=10)
        if response.ok:
            return response.json()
        sleep(delay * (attempt + 1))
    raise TimeoutError(url)
```

Uploads follow the same rule with their own limits:

```python
def upload_with_retry(session, path, tries):
    pause = 2
    for step in range(tries):
        result = session.get(path, timeout=60)
        if result.ok:
            return result.json()
        sleep(pause * (step + 1))
    raise TimeoutError(path)
```
