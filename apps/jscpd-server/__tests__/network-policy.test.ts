import { describe, it, expect } from "vitest";
import {
  isLocalBindHost,
  isWildcardBindHost,
  resolveAllowedHosts,
  resolveAllowedOrigins,
  toHostname,
} from "../src/server/network-policy";

const LOCALHOST_ALIASES = ["localhost", "127.0.0.1", "[::1]"];

describe("toHostname", () => {
  it("accepts every spelling of a hostname", () => {
    expect(toHostname("example.com")).toBe("example.com");
    expect(toHostname("example.com:8080")).toBe("example.com");
    expect(toHostname("https://example.com:8443")).toBe("example.com");
    expect(toHostname("  EXAMPLE.com  ")).toBe("example.com");
  });

  it("brackets bare IPv6 literals", () => {
    expect(toHostname("::1")).toBe("[::1]");
    expect(toHostname("[::1]")).toBe("[::1]");
    expect(toHostname("[::1]:3000")).toBe("[::1]");
    expect(toHostname("http://[::1]:3000")).toBe("[::1]");
    expect(toHostname("2001:db8::1")).toBe("[2001:db8::1]");
  });

  it("leaves an empty entry empty", () => {
    expect(toHostname("   ")).toBe("");
  });
});

describe("bind host classification", () => {
  it("recognizes wildcard binds", () => {
    for (const host of ["0.0.0.0", "::", "[::]", "*", ""]) {
      expect(isWildcardBindHost(host)).toBe(true);
    }
    expect(isWildcardBindHost("127.0.0.1")).toBe(false);
  });

  it("recognizes loopback binds in both IPv6 spellings", () => {
    for (const host of ["localhost", "127.0.0.1", "::1", "[::1]"]) {
      expect(isLocalBindHost(host)).toBe(true);
    }
    expect(isLocalBindHost("0.0.0.0")).toBe(false);
    expect(isLocalBindHost("jscpd.example.com")).toBe(false);
  });
});

describe("resolveAllowedHosts", () => {
  it("allows every local alias on a bare IPv6 loopback bind", () => {
    expect(resolveAllowedHosts("::1")).toEqual(
      expect.arrayContaining(LOCALHOST_ALIASES),
    );
  });

  it("extends rather than replaces the local aliases on a loopback bind", () => {
    const allowed = resolveAllowedHosts("127.0.0.1", ["jscpd.internal:8443"]);

    expect(allowed).toEqual(
      expect.arrayContaining([...LOCALHOST_ALIASES, "jscpd.internal"]),
    );
  });

  it("keeps a configured allowlist strict on an external bind", () => {
    expect(resolveAllowedHosts("0.0.0.0", ["jscpd.example.com"])).toEqual([
      "jscpd.example.com",
    ]);
  });

  it("applies no restriction on an unconfigured wildcard bind", () => {
    expect(resolveAllowedHosts("0.0.0.0")).toBeUndefined();
  });

  it("includes the concrete non-loopback bind host with configured extras", () => {
    expect(resolveAllowedHosts("jscpd.example.com", ["extra.internal"])).toEqual(
      ["jscpd.example.com", "extra.internal"],
    );
  });

  it("includes a concrete non-loopback bind host on its own", () => {
    expect(resolveAllowedHosts("jscpd.example.com")).toEqual([
      "jscpd.example.com",
    ]);
  });
});

describe("resolveAllowedOrigins", () => {
  it("always allows the local aliases", () => {
    expect(resolveAllowedOrigins("0.0.0.0")).toEqual(
      expect.arrayContaining(LOCALHOST_ALIASES),
    );
  });

  it("allows a concrete bind host and configured extras without duplicates", () => {
    const allowed = resolveAllowedOrigins("jscpd.example.com", [
      "https://ide.internal:8443",
      "localhost",
    ]);

    expect(allowed).toEqual(
      expect.arrayContaining([
        ...LOCALHOST_ALIASES,
        "jscpd.example.com",
        "ide.internal",
      ]),
    );
    expect(new Set(allowed).size).toBe(allowed.length);
  });
});
