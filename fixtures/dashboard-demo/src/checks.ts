import type { Parcel } from './rates';

export function checkParcel(parcel: Parcel): string[] {
  const problems: string[] = [];
  if (parcel.weightKg <= 0) {
    problems.push('weight must be positive');
  }
  const lines = [
    parcel.recipient.name.trim(),
    parcel.recipient.street.trim(),
    `${parcel.recipient.postcode.trim()} ${parcel.recipient.city.trim().toUpperCase()}`,
  ].filter((line) => line.length > 0);
  if (lines.length < 3) {
    problems.push('recipient address is incomplete');
  }
  return problems;
}

export function checkBatch(parcels: Parcel[]): number {
  return parcels.filter((parcel) => checkParcel(parcel).length > 0).length;
}
