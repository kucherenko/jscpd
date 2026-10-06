class GuestLog:
    """Who stayed when, for the front desk."""

    def __init__(self, db, cache, clock):
        self.db = db
        self.cache = cache
        self.clock = clock

    def load(self, entry_id):
        key = f"guest:{entry_id}"
        entry = self.cache.get(key)
        if entry is None:
            entry = self.db.fetch(self.table, entry_id)
            self.cache.set(key, entry, ttl=3600)
        return entry

    def check_in(self, guest, room):
        stamp = self.clock.now()
        self.db.insert(self.table, {"guest": guest, "room": room, "at": stamp})
        return stamp
