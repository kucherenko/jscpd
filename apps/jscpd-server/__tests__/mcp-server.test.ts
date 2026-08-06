import { describe, it, expect, beforeAll, afterAll } from "vitest";
import request from "supertest";
import path from "path";
import {
  CLIENT_CAPABILITIES_META_KEY,
  CLIENT_INFO_META_KEY,
  PROTOCOL_VERSION_META_KEY,
  SERVER_INFO_META_KEY,
} from "@modelcontextprotocol/server";
import { JscpdServer, startServer } from "../src/server";
import { MCP_MODERN_PROTOCOL_VERSION } from "../src/server/constants";

const LEGACY_PROTOCOL_VERSION = "2025-06-18";
const HEADER_MISMATCH_ERROR_CODE = -32020;
const INVALID_PARAMS_ERROR_CODE = -32602;
const METHOD_NOT_FOUND_ERROR_CODE = -32601;

const CLIENT_INFO = { name: "jscpd-test-client", version: "1.0.0" };

function modernEnvelope(protocolVersion = MCP_MODERN_PROTOCOL_VERSION) {
  return {
    [PROTOCOL_VERSION_META_KEY]: protocolVersion,
    [CLIENT_CAPABILITIES_META_KEY]: {},
    [CLIENT_INFO_META_KEY]: CLIENT_INFO,
  };
}

/**
 * The 2025-era stateless fallback answers over SSE, so a legacy response body
 * has to be lifted out of the event frames.
 */
function parseSseResult(text: string): any {
  const payload = text
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice("data:".length).trim())
    .find((line) => line.length > 0);

  expect(payload).toBeDefined();
  return JSON.parse(payload as string);
}

function bodyOf(response: request.Response): any {
  return response.headers["content-type"]?.includes("text/event-stream")
    ? parseSseResult(response.text)
    : response.body;
}

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
  function modernRequest(
    method: string,
    params: Record<string, unknown> = {},
    options: {
      id?: number;
      protocolVersion?: string;
      envelope?: Record<string, unknown> | null;
      methodHeader?: string | null;
      nameHeader?: string | null;
      versionHeader?: string | null;
    } = {},
  ) {
    const {
      id = 1,
      protocolVersion = MCP_MODERN_PROTOCOL_VERSION,
      envelope = modernEnvelope(protocolVersion),
      methodHeader = method,
      nameHeader,
      versionHeader = protocolVersion,
    } = options;

    let call = req
      .post("/mcp")
      .set("Content-Type", "application/json")
      .set("Accept", "application/json, text/event-stream");

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

    it("stamps cache fields on list results", async () => {
      const [tools, resources, templates] = await Promise.all([
        modernRequest("tools/list", {}, { id: 8 }),
        modernRequest("resources/list", {}, { id: 9 }),
        modernRequest("resources/templates/list", {}, { id: 10 }),
      ]);

      for (const response of [tools, resources, templates]) {
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
});
