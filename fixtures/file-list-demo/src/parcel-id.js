const ALPHABET = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';

export function parcelId(seed, length = 10) {
  let state = seed >>> 0;
  let id = '';
  for (let i = 0; i < length; i += 1) {
    state = (state * 1103515245 + 12345) >>> 0;
    id += ALPHABET[state % ALPHABET.length];
  }
  return id;
}

export function checkDigit(id) {
  let sum = 0;
  for (const char of id) {
    sum = (sum * 31 + ALPHABET.indexOf(char)) % 97;
  }
  return String(sum).padStart(2, '0');
}
