# Notifications

Each channel keeps an outbox of its own. Email goes out through SMTP:

```python
class EmailOutbox:
    def __init__(self, smtp, templates):
        self.smtp = smtp
        self.templates = templates

    def send(self, user, event):
        body = self.templates.render(event.kind, user=user)
        self.smtp.deliver(user.email, subject=event.title, body=body)
        return len(body)
```

Text messages take the same steps through a gateway:

```python
class SmsOutbox:
    def __init__(self, gateway, texts):
        self.gateway = gateway
        self.texts = texts

    def send(self, member, notice):
        message = self.texts.render(notice.kind, user=member)
        self.gateway.deliver(member.phone, subject=notice.title, body=message)
        return len(message)
```
