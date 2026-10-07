(ns library.shelf)

(defn refill [books publisher]
  (reduce (fn [requested book]
            (if (< (:quantity book) (:minimum book))
              (+ requested (order! publisher (:code book) (* 3 (:minimum book))))
              requested))
          0
          books))
