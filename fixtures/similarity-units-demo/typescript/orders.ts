import { Injectable } from '@nestjs/common';
import { createRouter, route } from './http';

export interface Order {
  id: string;
  customer: string;
  total: number;
}

export enum OrderState {
  Draft = 'draft',
  Paid = 'paid',
}

export type OrderPatch<T> = {
  [K in keyof T]?: T[K] extends Array<infer Line>
    ? Array<OrderPatch<Line>>
    : T[K] extends object
      ? OrderPatch<T[K]>
      : T[K] | null;
};

@Injectable()
export class OrdersService {
  private readonly cache = new Map<string, Order>();

  constructor(private readonly db: Database) {}

  async find(id: string): Promise<Order> {
    const cached = this.cache.get(id);
    if (cached) {
      return cached;
    }
    const order = await this.db.one('select * from orders where id = $1', [id]);
    this.cache.set(id, order);
    return order;
  }
}

export const ordersRouter = createRouter({
  prefix: '/orders',
  routes: [
    route.get('/:id', (req) => orders.find(req.params.id)),
    route.post('/', (req) => orders.create(req.body)),
  ],
  onError: (error) => logger.warn('orders', error),
});
