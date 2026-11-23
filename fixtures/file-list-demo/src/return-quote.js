export function returnQuote(parcel, zone, rates) {
  const weight = Math.max(parcel.weightKg, 0.1);
  const volume = parcel.lengthCm * parcel.widthCm * parcel.heightCm;
  const billable = Math.max(weight, volume / 5000);
  const band = rates.bands.find((entry) => billable <= entry.upToKg);
  if (!band) {
    throw new Error(`no rate band for ${billable.toFixed(2)} kg`);
  }
  let price = band.base + band.perKg * billable;
  if (zone.remote) {
    price += rates.remoteSurcharge;
  }
  if (parcel.fragile) {
    price *= 1 + rates.fragileShare;
  }
  return {
    zone: zone.code,
    billableKg: Number(billable.toFixed(2)),
    price: Math.round(price * 100) / 100,
    currency: rates.currency,
  };
}

export const returnWindowDays = (order) => (order.member ? 60 : 30);
