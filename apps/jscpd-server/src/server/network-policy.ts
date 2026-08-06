import {
  localhostAllowedHostnames,
  localhostAllowedOrigins,
} from "@modelcontextprotocol/server";

const WILDCARD_BIND_HOSTS = new Set(["", "0.0.0.0", "::", "[::]", "*"]);

/**
 * Reduces a user-supplied entry to the bare hostname the SDK guards compare
 * against, so `https://example.com:8080`, `example.com:8080` and `example.com`
 * are all accepted spellings. IPv6 keeps its brackets (`[::1]`), matching
 * {@link localhostAllowedHostnames}.
 */
export function toHostname(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }

  for (const candidate of [trimmed, `http://${trimmed}`]) {
    try {
      const { hostname } = new URL(candidate);
      if (hostname) {
        return hostname;
      }
    } catch {
      // Not a URL on this attempt; fall through to the next spelling.
    }
  }

  return trimmed.toLowerCase();
}

function unique(hostnames: string[]): string[] {
  return [...new Set(hostnames.filter((hostname) => hostname.length > 0))];
}

/** Whether a bind address covers every interface rather than one known host. */
export function isWildcardBindHost(host: string): boolean {
  return WILDCARD_BIND_HOSTS.has(host.trim().toLowerCase());
}

/** Whether a bind address is loopback-only. */
export function isLocalBindHost(host: string): boolean {
  return localhostAllowedHostnames().includes(toHostname(host));
}

/**
 * Origin hostnames the MCP endpoint accepts. The loopback names are always
 * allowed, a concrete bind address is allowed as itself, and deployments that
 * are reached from a browser under another name add it explicitly.
 */
export function resolveAllowedOrigins(
  bindHost: string,
  configured: string[] = [],
): string[] {
  return unique([
    ...localhostAllowedOrigins(),
    ...(isWildcardBindHost(bindHost) ? [] : [toHostname(bindHost)]),
    ...configured.map(toHostname),
  ]);
}

/**
 * Host header allowlist for the MCP endpoint, or `undefined` when no Host
 * restriction applies. A loopback bind gets the localhost allowlist for free;
 * a deliberate external bind is only restricted when the operator names the
 * hostnames, since the server cannot guess how it is addressed from outside.
 */
export function resolveAllowedHosts(
  bindHost: string,
  configured: string[] = [],
): string[] | undefined {
  const explicit = unique(configured.map(toHostname));
  if (explicit.length > 0) {
    return explicit;
  }

  return isLocalBindHost(bindHost) ? localhostAllowedHostnames() : undefined;
}
