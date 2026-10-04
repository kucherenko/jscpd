export function returnSlips(boxes, courier) {
  const slips = [];
  for (const box of boxes) {
    const kilos = Math.ceil(box.grams / 1000);
    const region = box.country === courier.home ? 'local' : 'remote';
    slips.push({ id: box.id, kilos, region, price: courier.rates[region] * kilos });
  }
  slips.sort((first, second) => second.price - first.price);
  return slips;
}
