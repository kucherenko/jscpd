export interface Parcel {
  weightKg: number;
  zone: string;
  express: boolean;
  fragile?: boolean;
  recipient: { name: string; street: string; city: string; postcode: string };
}

export function quoteShipment(parcel: Parcel): number {
  let price = 4;
  if (parcel.weightKg > 20) {
    price += 30;
  } else if (parcel.weightKg > 5) {
    price += 12;
  } else if (parcel.weightKg > 1) {
    price += 5;
  }
  switch (parcel.zone) {
    case 'domestic':
      break;
    case 'eu':
      price *= 1.4;
      break;
    case 'world':
      price *= 2.2;
      break;
  }
  if (parcel.express && parcel.zone !== 'world') {
    price += 9;
  }
  if (parcel.fragile || parcel.weightKg > 30) {
    price += 6;
  }
  return Math.round(price * 100) / 100;
}
