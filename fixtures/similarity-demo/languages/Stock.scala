object Stock {
  def restock(items: Seq[Item], supplier: Supplier): Int = {
    var ordered = 0
    for (item <- items) {
      if (item.quantity < item.minimum) {
        ordered += supplier.order(item.code, item.minimum * 2)
      }
    }
    ordered
  }
}
