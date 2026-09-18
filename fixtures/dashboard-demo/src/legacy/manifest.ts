import type { Parcel } from '../rates';

export function buildManifest(parcels: Parcel[], date: string): string {
  const rows = parcels.map((parcel, index) => {
    const weight = parcel.weightKg.toFixed(1);
    const service = parcel.express ? 'X' : 'S';
    return [index + 1, parcel.recipient.name, parcel.zone, weight, service].join(';');
  });
  return [`MANIFEST ${date}`, ...rows, `TOTAL ${parcels.length}`].join('\n');
}
