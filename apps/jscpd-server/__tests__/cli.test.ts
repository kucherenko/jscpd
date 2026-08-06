import { describe, it, expect, vi } from "vitest";
import http from "node:http";
import type { AddressInfo } from "node:net";
import path from "path";
import request from "supertest";
import { runServer } from "../src";

const FIXTURES = path.join(__dirname, "../../../fixtures/javascript");

async function freePort(): Promise<number> {
  const probe = http.createServer();
  await new Promise<void>((resolve) => probe.listen(0, "127.0.0.1", resolve));
  const { port } = probe.address() as AddressInfo;
  await new Promise<void>((resolve) => probe.close(() => resolve()));
  return port;
}

describe("server CLI", () => {
  it.each(["--allowed-origin", "--allowed-host"])(
    "rejects %s without a value instead of crashing",
    async (flag) => {
      const exit = vi.fn();
      const stderr = vi
        .spyOn(process.stderr, "write")
        .mockImplementation(() => true);

      const server = await runServer(["node", "jscpd-server", ".", flag], exit);
      const reported = stderr.mock.calls.map(([chunk]) => String(chunk)).join("");
      stderr.mockRestore();

      expect(server).toBeNull();
      expect(exit).toHaveBeenCalledWith(1);
      expect(reported).toContain(`option '${flag} <hostname>' argument missing`);
    },
  );

  it("collects repeated --allowed-origin values", async () => {
    const port = await freePort();
    const server = await runServer([
      "node",
      "jscpd-server",
      FIXTURES,
      "--port",
      String(port),
      "--host",
      "127.0.0.1",
      "--allowed-origin",
      "https://ide.internal:8443",
      "--allowed-origin",
      "https://docs.internal",
      "--min-lines",
      "5",
      "--min-tokens",
      "40",
    ]);

    expect(server).not.toBeNull();

    try {
      const agent = request(server!.getApp());
      const discover = (origin: string) =>
        agent
          .post("/mcp")
          .set("Content-Type", "application/json")
          .set("Accept", "application/json, text/event-stream")
          .set("Origin", origin)
          .set("MCP-Protocol-Version", "2026-07-28")
          .set("Mcp-Method", "server/discover")
          .send({
            jsonrpc: "2.0",
            id: 1,
            method: "server/discover",
            params: {
              _meta: {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
              },
            },
          });

      expect((await discover("https://ide.internal")).status).toBe(200);
      expect((await discover("https://docs.internal")).status).toBe(200);
      expect((await discover("https://evil.example.com")).status).toBe(403);
    } finally {
      await server!.stop();
    }
  }, 120_000);
});
