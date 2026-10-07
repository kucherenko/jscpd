<?php

class Shelf
{
    public function refill(array $books, Publisher $publisher): int
    {
        $requested = 0;
        foreach ($books as $book) {
            if ($book->quantity < $book->minimum) {
                $requested += $publisher->order($book->code, $book->minimum * 3);
            }
        }
        return $requested;
    }
}
