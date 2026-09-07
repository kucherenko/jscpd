export async function saveProfile(profile, store, clock) {
  const record = toRecord(profile);
  record.updatedAt = clock.now();
  record.version = (record.version || 0) + 1;
  record.normalizedEmail = profile.email.trim().toLowerCase();
  record.displayName = [profile.firstName, profile.lastName].filter(Boolean).join(" ");
  await store.put("profiles", record.id, record);
  await audit("save-profile", record.id, record.version, clock.now());
  await notify(profile.email, "profile-updated", { version: record.version });
  return { id: record.id, version: record.version, updatedAt: record.updatedAt };
}
