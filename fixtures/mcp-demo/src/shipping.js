export function parcelLabels(parcels, carrier) {
  const labels = [];
  for (const parcel of parcels) {
    const weight = Math.ceil(parcel.grams / 1000);
    const zone = parcel.country === carrier.home ? 'domestic' : 'abroad';
    labels.push({ id: parcel.id, weight, zone, price: carrier.rates[zone] * weight });
  }
  labels.sort((left, right) => right.price - left.price);
  return labels;
}
