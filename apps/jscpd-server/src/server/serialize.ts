/**
 * Optional GCF (Graph Compact Format) serialization for MCP tool responses.
 *
 * When enabled, structured data is encoded using GCF's tabular encoding
 * instead of JSON, saving ~56% of tokens on duplication results.
 *
 * Enable via CLI flag: jscpd-server --gcf
 * Or environment variable: JSCPD_OUTPUT_FORMAT=gcf
 *
 * Requires the optional @blackwell-systems/gcf package.
 */

let encodeGeneric: ((data: unknown) => string) | null = null;

try {
  const gcf = await import("@blackwell-systems/gcf");
  encodeGeneric = gcf.encodeGeneric;
} catch {
  // GCF is an optional dependency
}

let gcfEnabled: boolean | null = null;

/**
 * Check whether GCF output is enabled and available.
 * Returns true only when the `--gcf` flag or `JSCPD_OUTPUT_FORMAT=gcf`
 * env var is set AND the `@blackwell-systems/gcf` package is installed.
 */
export function isGcfEnabled(): boolean {
  if (gcfEnabled === null) {
    gcfEnabled =
      process.env.JSCPD_OUTPUT_FORMAT === "gcf" ||
      process.argv.includes("--gcf");
  }
  return gcfEnabled && encodeGeneric !== null;
}

/**
 * Serialize data as JSON or GCF depending on configuration.
 * Drop-in replacement for JSON.stringify(data, null, 2).
 */
export function serialize(data: unknown, indent = 2): string {
  if (isGcfEnabled()) {
    try {
      return encodeGeneric(data);
    } catch {
      // Fall back to JSON on any encoding error
    }
  }
  return JSON.stringify(data, null, indent);
}
