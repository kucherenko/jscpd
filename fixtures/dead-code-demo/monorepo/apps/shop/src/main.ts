import { badge } from '@demo/ui/badge';

const outOfStock = 3;
document.title = badge(`${outOfStock} items out of stock`, 'warning');
