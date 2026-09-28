// Parcel pricing, ported from python/shipping.py.

const BASE_BY_ZONE: Record<string, number> = { domestic: 490, europe: 1290, world: 2490 };

export function shippingCost(weightGrams: number, zone: string, express: boolean): number {
  const base = BASE_BY_ZONE[zone];
  if (base === undefined) {
    throw new Error(`unknown zone ${zone}`);
  }
  const kilograms = Math.ceil(weightGrams / 1000);
  const cost = base + Math.max(kilograms - 1, 0) * 150;
  return express ? cost * 2 : cost;
}
