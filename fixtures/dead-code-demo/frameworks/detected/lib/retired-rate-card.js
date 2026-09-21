// Priced parcels by weight band before the carrier API took that over.
const BANDS = [
  { upToGrams: 500, pence: 295 },
  { upToGrams: 2000, pence: 495 },
  { upToGrams: 10000, pence: 1150 },
];

export function quoteByWeight(grams) {
  const band = BANDS.find((candidate) => grams <= candidate.upToGrams);
  return band ? band.pence : null;
}
