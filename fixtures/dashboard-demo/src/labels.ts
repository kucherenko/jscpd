import type { Parcel } from './rates';

export function printLabel(parcel: Parcel): string {
  const lines = [
    parcel.recipient.name.trim(),
    parcel.recipient.street.trim(),
    `${parcel.recipient.postcode.trim()} ${parcel.recipient.city.trim().toUpperCase()}`,
  ].filter((line) => line.length > 0);
  const header = parcel.express ? 'EXPRESS' : 'STANDARD';
  return [header, ...lines].join('\n');
}
