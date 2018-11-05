export const SOURCES_DB = 'sources';
export const STATISTIC_DB = 'statistic';
export const HASHES_DB_PREFIX = 'hashes_';

export function getHashDbName(format: string): string {
  return HASHES_DB_PREFIX + format;
}
