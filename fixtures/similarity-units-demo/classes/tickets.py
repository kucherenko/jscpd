from support.storage import Collection


class TicketBoard:
    """Tickets of the help desk, kept in memory between requests."""

    collection = Collection("tickets", key="ticket_id")

    def __init__(self, storage, memo):
        self.storage = storage
        self.memo = memo

    def load(self, ticket_id):
        slot = f"ticket:{ticket_id}"
        ticket = self.memo.get(slot)
        if ticket is None:
            ticket = self.storage.fetch(self.collection, ticket_id)
            self.memo.set(slot, ticket, ttl=60)
        return ticket

    def cancel(self, ticket_id, note):
        ticket = self.load(ticket_id)
        ticket.status = "closed"
        ticket.reason = note
        self.storage.update(self.collection, ticket)
        self.memo.delete(f"ticket:{ticket_id}")
        return ticket
