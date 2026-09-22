pub fn render(order_id: u32, index: u32, of: u32) -> String {
    format!("ORDER {order_id} · PARCEL {index}/{of}")
}

pub fn render_return(order_id: u32) -> String {
    format!("RETURN {order_id}")
}
