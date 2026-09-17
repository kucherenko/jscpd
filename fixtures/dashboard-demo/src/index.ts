import { quoteShipment } from './rates';
import { checkParcel } from './checks';
import { printLabel } from './labels';

const parcel = {
  weightKg: 2.4,
  zone: 'eu',
  express: true,
  recipient: { name: 'Olena', street: 'Khreshchatyk 1', city: 'Kyiv', postcode: '01001' },
};

const problems = checkParcel(parcel);
if (problems.length === 0) {
  console.log(quoteShipment(parcel));
  console.log(printLabel(parcel));
}
