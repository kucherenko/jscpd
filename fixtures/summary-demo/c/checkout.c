#include <stdbool.h>
#include <stddef.h>

typedef struct {
    int quantity;
    double unit_price;
    bool gift_wrap;
} LineItem;

double line_total(const LineItem *item) {
    if (item == NULL || item->quantity <= 0) {
        return 0.0;
    }
    double total = item->quantity * item->unit_price;
    if (item->gift_wrap && total > 0.0) {
        total += 2.5;
    }
    return total;
}

int shipping_zone(const char *country) {
    switch (country[0]) {
        case 'D': return 1;
        case 'F': return 1;
        case 'U': return 3;
        default:  return 2;
    }
}

bool free_shipping(double subtotal, int zone) {
    return (zone == 1 && subtotal >= 50.0) || (zone == 2 && subtotal >= 80.0);
}
