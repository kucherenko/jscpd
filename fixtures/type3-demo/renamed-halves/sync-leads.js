export async function syncLeads(pipeline, mirror, audit) {
  const checkpoint = await mirror.readCheckpoint(LEAD_SCOPE);
  const chunk = await pipeline.listLeads({ since: checkpoint, limit: 200 });
  const dirty = chunk.items.filter((lead) => lead.updatedAt > checkpoint);
  const puts = dirty.map((lead) => mirror.putLead(lead));
  const purges = chunk.deleted.map((leadId) => mirror.purgeLead(leadId));
  if (dirty.length === 0 && purges.length === 0) return null;
  await Promise.all([...puts, ...purges]);
  await mirror.writeCheckpoint(LEAD_SCOPE, chunk.nextCheckpoint);
  const digest = { dirty: dirty.length, purges: purges.length, checkpoint: chunk.nextCheckpoint };
  audit.info(LEAD_SYNCED, digest);
  metrics.increment(LEAD_METRIC, digest.dirty + digest.purges);
  return digest;
}
