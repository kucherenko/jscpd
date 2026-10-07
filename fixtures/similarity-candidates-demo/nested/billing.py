import csv


def export_invoices(invoices, path, tax_rate):
    def invoice_row(invoice):
        total = sum(item.price * item.count for item in invoice.items)
        tax = round(total * tax_rate, 2)
        due = invoice.issued + invoice.terms
        label = invoice.customer.name.strip().title()
        return [invoice.number, label, total, tax, due.isoformat()]

    class Batch:
        def __init__(self, writer, size):
            self.writer = writer
            self.size = size
            self.rows = []

        def push(self, row):
            self.rows.append(row)
            if len(self.rows) >= self.size:
                self.writer.writerows(self.rows)
                self.rows.clear()
            return len(self.rows)

    with open(path, "w", newline="") as handle:
        batch = Batch(csv.writer(handle), 100)
        for invoice in sorted(invoices, key=lambda invoice: invoice.number):
            if invoice.status != "void":
                batch.push(invoice_row(invoice))
        batch.writer.writerows(batch.rows)
    return path
