(ns warehouse.stock)

(defn restock [items supplier]
  (reduce (fn [ordered item]
            (if (< (:quantity item) (:minimum item))
              (+ ordered (order! supplier (:code item) (* 2 (:minimum item))))
              ordered))
          0
          items))
