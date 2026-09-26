// Shapes of the JSON the backend sends and accepts.

export interface CartLine {
  sku: string;
  unitPriceCents: number;
  quantity: number;
}

export type Coupon = { kind: 'percent'; percent: number } | { kind: 'fixed'; amountCents: number };

export interface Article {
  id: number;
  slug: string;
  title: string;
  body: string;
  publishedAt: string;
}
