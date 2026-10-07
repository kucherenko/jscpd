import json


def publish_parcels(parcels, queue, fuel_rate):
    def parcel_record(parcel):
        weight = sum(box.mass * box.units for box in parcel.boxes)
        surcharge = round(weight * fuel_rate, 2)
        arrival = parcel.shipped + parcel.transit
        carrier = parcel.carrier.code.strip().upper()
        return [parcel.tracking, carrier, weight, surcharge, arrival.isoformat()]

    class Chunk:
        def __init__(self, sender, limit):
            self.sender = sender
            self.limit = limit
            self.items = []

        def add(self, item):
            self.items.append(item)
            if len(self.items) >= self.limit:
                self.sender.send(self.items)
                self.items.clear()
            return len(self.items)

    sent = 0
    chunk = Chunk(queue, 50)
    while parcels:
        parcel = parcels.pop()
        try:
            sent += chunk.add(json.dumps(parcel_record(parcel)))
        except ValueError:
            queue.dead_letter(parcel.tracking)
    queue.flush()
    return sent
