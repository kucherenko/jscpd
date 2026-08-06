import { describe, it, expect, beforeAll, afterAll, vi } from "vitest";
import request from "supertest";
import http from "node:http";
import type { AddressInfo } from "node:net";
import path from "path";
import {
  CLIENT_CAPABILITIES_META_KEY,
  CLIENT_INFO_META_KEY,
  PROTOCOL_VERSION_META_KEY,
  SERVER_INFO_META_KEY,
} from "@modelcontextprotocol/server";
import { JscpdServer, ServerOptions, startServer } from "../src/server";
import { MCP_MODERN_PROTOCOL_VERSION } from "../src/server/constants";

const LEGACY_PROTOCOL_VERSION = "2025-06-18";
const HEADER_MISMATCH_ERROR_CODE = -32020;
const INVALID_PARAMS_ERROR_CODE = -32602;
const METHOD_NOT_FOUND_ERROR_CODE = -32601;

const CLIENT_INFO = { name: "jscpd-test-client", version: "1.0.0" };

async function freePort(): Promise<number> {
  const probe = http.createServer();
  await new Promise<void>((resolve) => probe.listen(0, "127.0.0.1", resolve));
  const { port } = probe.address() as AddressInfo;
  await new Promise<void>((resolve) => probe.close(() => resolve()));
  return port;
}

/** Resolves when the port is free again, proving the listener has been closed. */
async function bindable(port: number): Promise<void> {
  const probe = http.createServer();
  try {
    await new Promise<void>((resolve, reject) => {
      probe.once("error", reject);
      probe.listen(port, "127.0.0.1", resolve);
    });
  } finally {
    await new Promise<void>((resolve) => probe.close(() => resolve()));
  }
}

function modernEnvelope(protocolVersion = MCP_MODERN_PROTOCOL_VERSION) {
  return {
    [PROTOCOL_VERSION_META_KEY]: protocolVersion,
    [CLIENT_CAPABILITIES_META_KEY]: {},
    [CLIENT_INFO_META_KEY]: CLIENT_INFO,
  };
}

/**
 * The 2025-era stateless fallback answers over SSE, so a legacy response body
 * has to be lifted out of the event frames. Follows the SSE parsing rules: a
 * stream carries several events separated by blank lines, an event's `data`
 * buffer is the concatenation of all of its `data:` fields joined with `\n`,
 * one optional leading space is stripped from each value, and comment lines
 * (`:` keep-alives) are ignored.
 */
function parseSseEvents(text: string): string[] {
  return text
    .split(/\r\n\r\n|\n\n|\r\r/)
    .map((frame) =>
      frame
        .split(/\r\n|\n|\r/)
        .filter((line) => line.startsWith("data:"))
        .map((line) => line.slice("data:".length).replace(/^ /, ""))
        .join("\n"),
    )
    .filter((data) => data.length > 0);
}

function parseSseResult(text: string): any {
  const messages = parseSseEvents(text).map((data) => JSON.parse(data));
  const response = messages.find(
    (message) => "result" in message || "error" in message,
  );

  expect(response).toBeDefined();
  return response;
}

function bodyOf(response: request.Response): any {
  return response.headers["content-type"]?.includes("text/event-stream")
    ? parseSseResult(response.text)
    : response.body;
}

describe("SSE frame parsing", () => {
  it("joins the data lines of one event and skips comments", () => {
    const stream = [
      ": keep-alive",
      "event: message",
      'data: {"jsonrpc":"2.0","id":1,',
      'data: "result":{"ok":true}}',
      "",
      "",
    ].join("\n");

    expect(parseSseResult(stream)).toEqual({
      jsonrpc: "2.0",
      id: 1,
      result: { ok: true },
    });
  });

  it("skips notification events and returns the response event", () => {
    const stream = [
      "event: message",
      'data: {"jsonrpc":"2.0","method":"notifications/progress","params":{}}',
      "",
      "event: message",
      'data: {"jsonrpc":"2.0","id":2,"result":{"tools":[]}}',
      "",
    ].join("\n");

    expect(parseSseResult(stream)).toEqual({
      jsonrpc: "2.0",
      id: 2,
      result: { tools: [] },
    });
  });
});

describe("MCP Server Integration", () => {
  let server: JscpdServer;
  let req: ReturnType<typeof request>;
  const fixturesDir = path.join(__dirname, "../../../fixtures");
  const jscpdOptions = {
    minLines: 5,
    minTokens: 40,
  };
  const port = 3002;

  /**
   * Sends a 2026-07-28 request: a direct, stateless exchange carrying the
   * per-request `_meta` envelope plus the matching standard headers. There is
   * no handshake and no session id.
   */
  function sendModern(
    agent: ReturnType<typeof request>,
    method: string,
    params: Record<string, unknown> = {},
    options: {
      id?: number;
      protocolVersion?: string;
      envelope?: Record<string, unknown> | null;
      methodHeader?: string | null;
      nameHeader?: string | null;
      versionHeader?: string | null;
      origin?: string;
      hostHeader?: string;
    } = {},
  ) {
    const {
      id = 1,
      protocolVersion = MCP_MODERN_PROTOCOL_VERSION,
      envelope = modernEnvelope(protocolVersion),
      methodHeader = method,
      nameHeader,
      versionHeader = protocolVersion,
      origin,
      hostHeader,
    } = options;

    let call = agent
      .post("/mcp")
      .set("Content-Type", "application/json")
      .set("Accept", "application/json, text/event-stream");

    if (origin !== undefined) {
      call = call.set("Origin", origin);
    }
    if (hostHeader !== undefined) {
      call = call.set("Host", hostHeader);
    }
    if (versionHeader !== null) {
      call = call.set("MCP-Protocol-Version", versionHeader);
    }
    if (methodHeader !== null) {
      call = call.set("Mcp-Method", methodHeader);
    }

    const implicitName = params.name ?? params.uri;
    const name =
      nameHeader === undefined
        ? typeof implicitName === "string"
          ? implicitName
          : null
        : nameHeader;
    if (name !== null) {
      call = call.set("Mcp-Name", name);
    }

    return call.send({
      jsonrpc: "2.0",
      id,
      method,
      params: envelope === null ? params : { ...params, _meta: envelope },
    });
  }

  function modernRequest(
    method: string,
    params: Record<string, unknown> = {},
    options: Parameters<typeof sendModern>[3] = {},
  ) {
    return sendModern(req, method, params, options);
  }

  /** Sends a 2025-era request: no envelope, no modern headers. */
  function legacyRequest(
    method: string,
    params: Record<string, unknown> = {},
    id = 1,
  ) {
    return req
      .post("/mcp")
      .set("Content-Type", "application/json")
      .set("Accept", "application/json, text/event-stream")
      .send({ jsonrpc: "2.0", id, method, params });
  }

  beforeAll(async () => {
    server = await startServer(fixturesDir, {
      port,
      jscpdOptions,
    });
    req = request(server.getApp());
  });

  afterAll(async () => {
    await server.stop();
  });

  describe("modern protocol revision 2026-07-28", () => {
    it("answers server/discover without any handshake", async () => {
      const response = await modernRequest("server/discover");

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty("jsonrpc", "2.0");
      expect(response.body).toHaveProperty("id", 1);
      expect(response.body.result.supportedVersions).toContain(
        MCP_MODERN_PROTOCOL_VERSION,
      );
      expect(response.body.result.capabilities).toHaveProperty("tools");
      expect(response.body.result.capabilities).toHaveProperty("resources");
      expect(response.body.result._meta[SERVER_INFO_META_KEY]).toMatchObject({
        name: "jscpd-server",
      });
      expect(response.body.result.ttlMs).toBe(300_000);
      expect(response.body.result.cacheScope).toBe("public");
    });

    it("serves direct requests without initialize and without a session id", async () => {
      const response = await modernRequest("tools/list", {}, { id: 2 });

      expect(response.status).toBe(200);
      expect(response.headers["mcp-session-id"]).toBeUndefined();
      expect(response.body.result.tools.map((tool: any) => tool.name)).toEqual(
        expect.arrayContaining([
          "check_duplication",
          "get_statistics",
          "check_current_directory",
        ]),
      );
    });

    it("rejects initialize on the modern era", async () => {
      const response = await modernRequest(
        "initialize",
        { protocolVersion: MCP_MODERN_PROTOCOL_VERSION, capabilities: {} },
        { id: 3 },
      );

      expect(response.body.error.code).toBe(METHOD_NOT_FOUND_ERROR_CODE);
    });

    it("stamps resultType on every result", async () => {
      const [discover, tools, resources, call] = await Promise.all([
        modernRequest("server/discover", {}, { id: 4 }),
        modernRequest("tools/list", {}, { id: 5 }),
        modernRequest("resources/list", {}, { id: 6 }),
        modernRequest(
          "tools/call",
          { name: "get_statistics", arguments: {} },
          { id: 7 },
        ),
      ]);

      for (const response of [discover, tools, resources, call]) {
        expect(response.status).toBe(200);
        expect(typeof response.body.result.resultType).toBe("string");
      }
    });

    it("stamps cache fields on every cacheable result", async () => {
      const [discover, tools, resources, templates] = await Promise.all([
        modernRequest("server/discover", {}, { id: 21 }),
        modernRequest("tools/list", {}, { id: 8 }),
        modernRequest("resources/list", {}, { id: 9 }),
        modernRequest("resources/templates/list", {}, { id: 10 }),
      ]);

      for (const response of [discover, tools, resources, templates]) {
        expect(response.status).toBe(200);
        expect(response.body.result.resultType).toBe("complete");
        expect(response.body.result.ttlMs).toBe(300_000);
        expect(response.body.result.cacheScope).toBe("public");
      }
    });

    it("stamps the per-resource cache hint on read results", async () => {
      const response = await modernRequest(
        "resources/read",
        { uri: "jscpd://statistics" },
        { id: 11 },
      );

      expect(response.status).toBe(200);
      expect(response.body.result.contents[0]).toHaveProperty(
        "uri",
        "jscpd://statistics",
      );
      expect(response.body.result.ttlMs).toBe(5_000);
      expect(response.body.result.cacheScope).toBe("private");
      expect(
        JSON.parse(response.body.result.contents[0].text),
      ).toHaveProperty("statistics");
    });

    it("runs the check_duplication tool with an inline recheck", async () => {
      const response = await modernRequest(
        "tools/call",
        {
          name: "check_duplication",
          arguments: {
            code: "function test() { console.log('hello'); }",
            format: "javascript",
            recheck: true,
          },
        },
        { id: 12 },
      );

      expect(response.status).toBe(200);
      expect(response.body.result.content[0].text).toContain("duplications");
      expect(response.body.result.content[0].text).toContain(
        "totalDuplications",
      );
    }, 120_000);

    it("runs the check_current_directory tool", async () => {
      const response = await modernRequest(
        "tools/call",
        { name: "check_current_directory", arguments: {} },
        { id: 13 },
      );

      expect(response.status).toBe(200);
      expect(
        JSON.parse(response.body.result.content[0].text),
      ).toHaveProperty("statistics");
    }, 120_000);

    it("requires the Mcp-Method header", async () => {
      const response = await modernRequest(
        "tools/list",
        {},
        { id: 14, methodHeader: null },
      );

      expect(response.status).toBe(400);
      expect(response.body.error.code).toBe(HEADER_MISMATCH_ERROR_CODE);
    });

    it("rejects an Mcp-Method header that disagrees with the body", async () => {
      const response = await modernRequest(
        "tools/list",
        {},
        { id: 15, methodHeader: "resources/list" },
      );

      expect(response.status).toBe(400);
      expect(response.body.error.code).toBe(HEADER_MISMATCH_ERROR_CODE);
    });

    it("requires an Mcp-Name header matching the called tool", async () => {
      const missing = await modernRequest(
        "tools/call",
        { name: "get_statistics", arguments: {} },
        { id: 16, nameHeader: null },
      );
      const mismatched = await modernRequest(
        "tools/call",
        { name: "get_statistics", arguments: {} },
        { id: 17, nameHeader: "check_duplication" },
      );

      expect(missing.status).toBe(400);
      expect(missing.body.error.code).toBe(HEADER_MISMATCH_ERROR_CODE);
      expect(mismatched.status).toBe(400);
      expect(mismatched.body.error.code).toBe(HEADER_MISMATCH_ERROR_CODE);
    });

    it("rejects an MCP-Protocol-Version header that disagrees with the envelope", async () => {
      const response = await modernRequest(
        "tools/list",
        {},
        { id: 18, versionHeader: "2027-01-01" },
      );

      expect(response.status).toBe(400);
      expect(response.body.error.code).toBe(HEADER_MISMATCH_ERROR_CODE);
    });

    it("rejects a modern header without the per-request envelope", async () => {
      const response = await modernRequest(
        "tools/list",
        {},
        { id: 19, envelope: null },
      );

      expect(response.status).toBe(400);
      expect(response.body.error.code).toBe(INVALID_PARAMS_ERROR_CODE);
    });

    it("rejects an unsupported modern revision", async () => {
      const response = await modernRequest(
        "tools/list",
        {},
        { id: 20, protocolVersion: "2030-01-01" },
      );

      expect(response.status).toBe(400);
      expect(response.body.error.data.supported).toContain(
        MCP_MODERN_PROTOCOL_VERSION,
      );
    });
  });

  describe("legacy 2025-era compatibility", () => {
    it("still answers the initialize handshake", async () => {
      const response = await legacyRequest("initialize", {
        protocolVersion: LEGACY_PROTOCOL_VERSION,
        capabilities: {},
        clientInfo: CLIENT_INFO,
      });

      expect(response.status).toBe(200);
      const body = bodyOf(response);
      expect(body).toHaveProperty("jsonrpc", "2.0");
      expect(body.result.serverInfo.name).toBe("jscpd-server");
      expect(body.result.protocolVersion).toBe(LEGACY_PROTOCOL_VERSION);
    });

    it("serves the same tools without a session id", async () => {
      const response = await legacyRequest("tools/list", {}, 2);

      expect(response.status).toBe(200);
      const body = bodyOf(response);
      expect(body.result.tools.map((tool: any) => tool.name)).toEqual(
        expect.arrayContaining([
          "check_duplication",
          "get_statistics",
          "check_current_directory",
        ]),
      );
      expect(body.result).not.toHaveProperty("resultType");
    });

    it("serves tool calls and resources", async () => {
      const call = await legacyRequest(
        "tools/call",
        { name: "get_statistics", arguments: {} },
        3,
      );
      const read = await legacyRequest(
        "resources/read",
        { uri: "jscpd://statistics" },
        4,
      );

      expect(JSON.parse(bodyOf(call).result.content[0].text)).toHaveProperty(
        "statistics",
      );
      const contents = bodyOf(read).result.contents;
      expect(contents).toHaveLength(1);
      expect(contents[0]).toHaveProperty("uri", "jscpd://statistics");
      expect(JSON.parse(contents[0].text)).toHaveProperty("statistics");
    });

    it("returns 405 for the session-oriented GET and DELETE operations", async () => {
      await req.get("/mcp").expect(405);
      await req.delete("/mcp").expect(405);
    });

    it("returns 400 for invalid JSON", async () => {
      await req
        .post("/mcp")
        .set("Content-Type", "application/json")
        .send("invalid-json")
        .expect(400);
    });
  });

  describe("endpoint lifecycle", () => {
    const javascriptFixtures = path.join(fixturesDir, "javascript");

    it("serves MCP again after a stop/start cycle", async () => {
      const restarted = new JscpdServer(javascriptFixtures, {
        port: 0,
        jscpdOptions,
      });
      const agent = request(restarted.getApp());

      try {
        await restarted.start();
        const before = await sendModern(agent, "server/discover");
        expect(before.status).toBe(200);

        await restarted.stop();
        await restarted.start();

        const after = await sendModern(agent, "server/discover", {}, { id: 2 });
        expect(after.status).toBe(200);
        expect(after.body.result.supportedVersions).toContain(
          MCP_MODERN_PROTOCOL_VERSION,
        );

        const call = await sendModern(
          agent,
          "tools/call",
          { name: "get_statistics", arguments: {} },
          { id: 3 },
        );
        expect(call.status).toBe(200);
        expect(JSON.parse(call.body.result.content[0].text)).toHaveProperty(
          "statistics",
        );
      } finally {
        await restarted.stop();
      }
    }, 120_000);

    it("reports the endpoint as unavailable while the server is stopped", async () => {
      const stopped = new JscpdServer(javascriptFixtures, {
        port: 0,
        jscpdOptions,
      });
      const agent = request(stopped.getApp());

      await stopped.start();
      await stopped.stop();

      const response = await sendModern(agent, "server/discover");
      expect(response.status).toBe(503);
      expect(response.body.error.message).toContain("MCP endpoint is closed");
    }, 120_000);

    it("rolls back an asynchronous listen failure and stays restartable", async () => {
      const blocker = http.createServer();
      await new Promise<void>((resolve) =>
        blocker.listen(0, "127.0.0.1", resolve),
      );
      const busyPort = (blocker.address() as AddressInfo).port;

      const failing = new JscpdServer(javascriptFixtures, {
        port: busyPort,
        host: "127.0.0.1",
        jscpdOptions,
      });
      const agent = request(failing.getApp());

      try {
        await expect(failing.start()).rejects.toMatchObject({
          code: "EADDRINUSE",
        });

        const rejected = await sendModern(agent, "server/discover");
        expect(rejected.status).toBe(503);

        await expect(failing.stop()).resolves.toBeUndefined();
      } finally {
        await new Promise<void>((resolve) => blocker.close(() => resolve()));
      }

      await failing.start();
      try {
        const response = await sendModern(agent, "server/discover");
        expect(response.status).toBe(200);
      } finally {
        await failing.stop();
      }
    }, 120_000);

    it("rolls back when the initial scan fails and preserves the error", async () => {
      const failing = new JscpdServer(javascriptFixtures, {
        port: 0,
        jscpdOptions,
      });
      const agent = request(failing.getApp());
      const scanFailure = new Error("scan exploded");
      const initialize = vi
        .spyOn(failing.getService(), "initialize")
        .mockRejectedValueOnce(scanFailure);

      await expect(failing.start()).rejects.toBe(scanFailure);
      initialize.mockRestore();

      const rejected = await sendModern(agent, "server/discover");
      expect(rejected.status).toBe(503);

      await expect(failing.stop()).resolves.toBeUndefined();

      await failing.start();
      try {
        expect((await sendModern(agent, "server/discover")).status).toBe(200);
      } finally {
        await failing.stop();
      }
    }, 120_000);

    it("stops accepting connections before tearing the MCP endpoint down", async () => {
      const running = new JscpdServer(javascriptFixtures, {
        port: 0,
        host: "127.0.0.1",
        jscpdOptions,
      });
      await running.start();

      const order: string[] = [];
      const internals = running as unknown as {
        closeHttpServer(): Promise<Error | undefined>;
        closeMcpEndpoint(): Promise<void>;
      };
      const closeHttpServer = internals.closeHttpServer.bind(running);
      const closeMcpEndpoint = internals.closeMcpEndpoint.bind(running);

      vi.spyOn(internals, "closeHttpServer").mockImplementation(() => {
        order.push("listener");
        return closeHttpServer();
      });
      vi.spyOn(internals, "closeMcpEndpoint").mockImplementation(() => {
        order.push("endpoint");
        return closeMcpEndpoint();
      });

      await running.stop();

      expect(order).toEqual(["listener", "endpoint"]);
    }, 120_000);

    it("frees the port on stop", async () => {
      const port = await freePort();
      const running = new JscpdServer(javascriptFixtures, {
        port,
        host: "127.0.0.1",
        jscpdOptions,
      });
      await running.start();
      const agent = request(running.getApp());

      expect((await sendModern(agent, "server/discover")).status).toBe(200);

      await running.stop();

      await expect(bindable(port)).resolves.toBeUndefined();
    }, 120_000);
  });

  describe("shutdown failure handling", () => {
    const javascriptFixtures = path.join(fixturesDir, "javascript");

    interface ShutdownInternals {
      mcp: { close(): Promise<void> };
      closeHttpServer(): Promise<Error | undefined>;
    }

    async function startOnFreePort(): Promise<{
      server: JscpdServer;
      port: number;
    }> {
      const port = await freePort();
      const server = new JscpdServer(javascriptFixtures, {
        port,
        host: "127.0.0.1",
        jscpdOptions,
      });
      await server.start();
      return { server, port };
    }

    /** Makes the HTTP drain report a failure while still closing the listener. */
    function failDrain(server: JscpdServer, failure: Error): void {
      const internals = server as unknown as ShutdownInternals;
      const closeHttpServer = internals.closeHttpServer.bind(server);
      vi.spyOn(internals, "closeHttpServer").mockImplementation(async () => {
        await closeHttpServer();
        return failure;
      });
    }

    /** Makes the service close report a failure while still releasing the store. */
    function failServiceClose(server: JscpdServer, failure: Error): void {
      const service = server.getService();
      const close = service.close.bind(service);
      vi.spyOn(service, "close").mockImplementation(async () => {
        await close();
        throw failure;
      });
    }

    function failEndpointClose(server: JscpdServer, failure: Error): void {
      const internals = server as unknown as ShutdownInternals;
      vi.spyOn(internals.mcp, "close").mockRejectedValue(failure);
    }

    it("still drains the listener and closes the service when the endpoint close fails", async () => {
      const { server, port } = await startOnFreePort();
      const failure = new Error("endpoint close exploded");

      failEndpointClose(server, failure);
      const serviceClose = vi.spyOn(server.getService(), "close");

      await expect(server.stop()).rejects.toBe(failure);

      expect(serviceClose).toHaveBeenCalledTimes(1);
      expect(server.getService().getState().statistics).toBeNull();
      await expect(bindable(port)).resolves.toBeUndefined();
      expect(
        (await sendModern(request(server.getApp()), "server/discover")).status,
      ).toBe(503);
    }, 120_000);

    it("keeps reporting a drain failure once every step has run", async () => {
      const { server, port } = await startOnFreePort();
      const failure = new Error("drain exploded");

      failDrain(server, failure);
      const serviceClose = vi.spyOn(server.getService(), "close");

      await expect(server.stop()).rejects.toBe(failure);

      expect(serviceClose).toHaveBeenCalledTimes(1);
      expect(server.getService().getState().statistics).toBeNull();
      await expect(bindable(port)).resolves.toBeUndefined();
    }, 120_000);

    it("reports a service close failure after the listener and endpoint are released", async () => {
      const { server, port } = await startOnFreePort();
      const failure = new Error("service close exploded");

      failServiceClose(server, failure);

      await expect(server.stop()).rejects.toBe(failure);

      await expect(bindable(port)).resolves.toBeUndefined();
      expect(
        (await sendModern(request(server.getApp()), "server/discover")).status,
      ).toBe(503);
    }, 120_000);

    it("prefers the endpoint failure when every step fails", async () => {
      const { server, port } = await startOnFreePort();
      const endpointFailure = new Error("endpoint close exploded");
      const drainFailure = new Error("drain exploded");
      const serviceFailure = new Error("service close exploded");

      failEndpointClose(server, endpointFailure);
      failDrain(server, drainFailure);
      failServiceClose(server, serviceFailure);

      await expect(server.stop()).rejects.toBe(endpointFailure);

      expect(server.getService().getState().statistics).toBeNull();
      await expect(bindable(port)).resolves.toBeUndefined();
    }, 120_000);
  });

  describe("DNS rebinding protection", () => {
    const javascriptFixtures = path.join(fixturesDir, "javascript");

    async function withServer(
      options: ServerOptions,
      assertions: (agent: ReturnType<typeof request>) => Promise<void>,
    ): Promise<void> {
      const guarded = new JscpdServer(javascriptFixtures, {
        port: 0,
        jscpdOptions,
        ...options,
      });
      await guarded.start();
      try {
        await assertions(request(guarded.getApp()));
      } finally {
        await guarded.stop();
      }
    }

    it("rejects a hostile Origin with 403", async () => {
      await withServer({}, async (agent) => {
        const response = await sendModern(agent, "server/discover", {}, {
          origin: "http://evil.example.com",
        });

        expect(response.status).toBe(403);
        expect(response.body.error.message).toContain("Invalid Origin");
        expect(response.body.id).toBeNull();
      });
    }, 120_000);

    it("accepts a loopback Origin and requests that send none", async () => {
      await withServer({}, async (agent) => {
        const withOrigin = await sendModern(agent, "server/discover", {}, {
          origin: "http://localhost:3000",
        });
        const withoutOrigin = await sendModern(agent, "server/discover", {}, {
          id: 2,
        });

        expect(withOrigin.status).toBe(200);
        expect(withoutOrigin.status).toBe(200);
      });
    }, 120_000);

    it("accepts a configured extra Origin", async () => {
      await withServer(
        { allowedOrigins: ["https://jscpd.internal:8443"] },
        async (agent) => {
          const configured = await sendModern(agent, "server/discover", {}, {
            origin: "https://jscpd.internal",
          });
          const hostile = await sendModern(agent, "server/discover", {}, {
            id: 2,
            origin: "https://jscpd.internal.evil.com",
          });

          expect(configured.status).toBe(200);
          expect(hostile.status).toBe(403);
        },
      );
    }, 120_000);

    it("rejects a hostile Host header on a loopback bind", async () => {
      await withServer({ host: "127.0.0.1" }, async (agent) => {
        const hostile = await sendModern(agent, "server/discover", {}, {
          hostHeader: "evil.example.com",
        });
        const local = await sendModern(agent, "server/discover", {}, { id: 2 });

        expect(hostile.status).toBe(403);
        expect(hostile.body.error.message).toContain("Invalid Host");
        expect(local.status).toBe(200);
      });
    }, 120_000);

    it("leaves the Host header unrestricted on a deliberate external bind", async () => {
      await withServer({ host: "0.0.0.0" }, async (agent) => {
        const response = await sendModern(agent, "server/discover", {}, {
          hostHeader: "jscpd.example.com",
        });

        expect(response.status).toBe(200);
      });
    }, 120_000);

    it("honours a configured Host allowlist on an external bind", async () => {
      await withServer(
        { host: "0.0.0.0", allowedHosts: ["jscpd.example.com"] },
        async (agent) => {
          const allowed = await sendModern(agent, "server/discover", {}, {
            hostHeader: "jscpd.example.com",
          });
          const hostile = await sendModern(agent, "server/discover", {}, {
            id: 2,
            hostHeader: "evil.example.com",
          });

          expect(allowed.status).toBe(200);
          expect(hostile.status).toBe(403);
        },
      );
    }, 120_000);

    it("keeps the local aliases reachable on an IPv6 loopback bind", async () => {
      await withServer({ host: "::1" }, async (agent) => {
        const responses = await Promise.all(
          ["localhost", "127.0.0.1", "[::1]"].map((hostHeader, index) =>
            sendModern(agent, "server/discover", {}, {
              id: index + 1,
              hostHeader,
            }),
          ),
        );
        const hostile = await sendModern(agent, "server/discover", {}, {
          id: 4,
          hostHeader: "evil.example.com",
        });

        for (const response of responses) {
          expect(response.status).toBe(200);
        }
        expect(hostile.status).toBe(403);
      });
    }, 120_000);

    it("keeps the local aliases reachable when extra hosts are configured", async () => {
      await withServer(
        { host: "127.0.0.1", allowedHosts: ["jscpd.internal"] },
        async (agent) => {
          const responses = await Promise.all(
            ["localhost", "127.0.0.1", "[::1]", "jscpd.internal"].map(
              (hostHeader, index) =>
                sendModern(agent, "server/discover", {}, {
                  id: index + 1,
                  hostHeader,
                }),
            ),
          );
          const hostile = await sendModern(agent, "server/discover", {}, {
            id: 5,
            hostHeader: "evil.example.com",
          });

          for (const response of responses) {
            expect(response.status).toBe(200);
          }
          expect(hostile.status).toBe(403);
        },
      );
    }, 120_000);
  });
});
