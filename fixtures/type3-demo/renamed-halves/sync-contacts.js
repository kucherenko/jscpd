export async function syncContacts(crm, cache, log) {
  const marker = await cache.readMarker(CONTACT_SCOPE);
  const batch = await crm.listContacts({ since: marker, limit: 200 });
  const touched = batch.items.filter((contact) => contact.updatedAt > marker);
  const writes = touched.map((contact) => cache.putContact(contact));
  const drops = batch.deleted.map((contactId) => cache.dropContact(contactId));
  await Promise.all([...writes, ...drops]);
  await cache.writeMarker(CONTACT_SCOPE, batch.nextMarker);
  const summary = { touched: touched.length, drops: drops.length, marker: batch.nextMarker };
  log.info(CONTACT_SYNCED, summary);
  metrics.increment(CONTACT_METRIC, summary.touched + summary.drops);
  return summary;
}
