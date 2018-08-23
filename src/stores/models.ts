export const CLONES_DB = 'clones';
export const SOURCES_DB = 'sources';
export const SOURCES_CLONES_DB = 'sources-clones';
export const STATISTIC_DB = 'statistic';
export const HASHES_DB_PREFIX = 'hashes.';
export const SOURCES_HASGES_DB_PREFIX = 'sources-hashes.';

export function getSourcesHashDbName(format: string): string {
  return SOURCES_HASGES_DB_PREFIX + format;
}

export function getHashDbName(format: string): string {
  return HASHES_DB_PREFIX + format;
}
