<?php

class Stock
{
    public function restock(array $items, Supplier $supplier): int
    {
        $ordered = 0;
        foreach ($items as $item) {
            if ($item->quantity < $item->minimum) {
                $ordered += $supplier->order($item->code, $item->minimum * 2);
            }
        }
        return $ordered;
    }
}
