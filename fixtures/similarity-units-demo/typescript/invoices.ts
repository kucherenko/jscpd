import { Injectable } from '@nestjs/common';
import { createRouter, route } from './http';

export interface Invoice {
  number: string;
  vendor: string;
  amount: number;
}

export enum InvoiceState {
  Open = 'open',
  Settled = 'settled',
}

export type InvoiceChanges<Shape> = {
  [Key in keyof Shape]?: Shape[Key] extends Array<infer Entry>
    ? Array<InvoiceChanges<Entry>>
    : Shape[Key] extends object
      ? InvoiceChanges<Shape[Key]>
      : Shape[Key] | null;
};

@Injectable()
export class InvoicesService {
  private readonly memo = new Map<string, Invoice>();

  constructor(private readonly store: Database) {}

  async lookup(number: string): Promise<Invoice> {
    const hit = this.memo.get(number);
    if (hit) {
      return hit;
    }
    const invoice = await this.store.one('select * from invoices where number = $1', [number]);
    this.memo.set(number, invoice);
    return invoice;
  }
}

export const invoicesRouter = createRouter({
  prefix: '/invoices',
  routes: [
    route.get('/:number', (req) => invoices.lookup(req.params.number)),
    route.post('/', (req) => invoices.create(req.body)),
  ],
  onError: (failure) => logger.warn('invoices', failure),
});
