interface Address {
  street: string;
  city?: string;
  postcode?: string;
}

interface Customer {
  name: string;
  email?: string;
  address?: Address;
}

export function shippingLabel(customer: Customer): string {
  const city = customer.address?.city ?? 'unknown city';
  return `${customer.name}, ${city}`;
}

export const hasContact = (customer: Customer): boolean =>
  Boolean(customer.email) || Boolean(customer.address?.postcode);
