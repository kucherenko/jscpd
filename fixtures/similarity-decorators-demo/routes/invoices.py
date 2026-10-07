from fastapi import APIRouter, Depends, HTTPException

router = APIRouter()


@router.delete("/invoices/{invoice_id}", status_code=204)
def drop_invoice(invoice_id: int, db=Depends(get_db)):
    invoice = db.query(Invoice).filter(Invoice.id == invoice_id).first()
    if invoice is None:
        raise HTTPException(status_code=404, detail="Invoice not found")
    invoice.deleted += 1
    db.commit()
    return invoice
