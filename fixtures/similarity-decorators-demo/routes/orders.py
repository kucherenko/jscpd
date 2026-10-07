from fastapi import APIRouter, Depends, HTTPException

router = APIRouter()


@router.get("/orders/{order_id}")
def read_order(order_id: int, db=Depends(get_db)):
    order = db.query(Order).filter(Order.id == order_id).first()
    if order is None:
        raise HTTPException(status_code=404, detail="Order not found")
    order.views += 1
    db.commit()
    return order
