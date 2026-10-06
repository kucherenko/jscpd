from hotel.db import Table


class BookingStore:
    """Bookings of the hotel, read through a cache."""

    table = Table("bookings", key="booking_id")

    def __init__(self, db, cache):
        self.db = db
        self.cache = cache

    def load(self, booking_id):
        key = f"booking:{booking_id}"
        booking = self.cache.get(key)
        if booking is None:
            booking = self.db.fetch(self.table, booking_id)
            self.cache.set(key, booking, ttl=300)
        return booking

    def cancel(self, booking_id, reason):
        booking = self.load(booking_id)
        booking.status = "cancelled"
        booking.reason = reason
        self.db.update(self.table, booking)
        self.cache.delete(f"booking:{booking_id}")
        return booking
