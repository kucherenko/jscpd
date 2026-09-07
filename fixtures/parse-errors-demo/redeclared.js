export async function saveProfile(profile, store, clock) {
  const record = toRecord(profile);
  record.updatedAt = clock.now();
  record.version = (record.version || 0) + 1;
  record.normalizedEmail = profile.email.trim().toLowerCase();
  record.displayName = [profile.firstName, profile.lastName].filter(Boolean).join(" ");
  if (!record.id) throw new Error("cannot save a profile without an id");
  await store.put("profiles", record.id, record);
  await audit("save-profile", record.id, record.version, clock.now());
  await notify(profile.email, "profile-updated", { version: record.version });
  return { id: record.id, version: record.version, updatedAt: record.updatedAt };
}

// A second declaration of the same export: a parse error for oxc, but the
// tokens are the same, so the file must still be compared with valid.js.
export async function saveProfile(profile, store, clock) {
  const record = toRecord(profile);
  record.updatedAt = clock.now();
  record.version = (record.version || 0) + 1;
  record.normalizedEmail = profile.email.trim().toLowerCase();
  record.displayName = [profile.firstName, profile.lastName].filter(Boolean).join(" ");
  return null;
}
