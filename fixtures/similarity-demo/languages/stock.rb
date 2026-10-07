class Stock
  def restock(items, supplier)
    ordered = 0
    items.each do |item|
      if item.quantity < item.minimum
        ordered += supplier.order(item.code, item.minimum * 2)
      end
    end
    ordered
  end
end
