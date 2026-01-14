import { describe, it, expect, beforeAll, afterAll } from "vitest";
import request from "supertest";
import path from "path";
import { JscpdServer, startServer } from "../src/server";
import { randomUUID } from "node:crypto";

describe("MCP Server Integration", () => {
  let server: JscpdServer;
  let req: request.SuperTest<request.Test>;
  const fixturesDir = path.join(__dirname, "../../../fixtures");
  const jscpdOptions = {
    minLines: 5,
    minTokens: 40,
  };
  const port = 3002;

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

  describe("POST /mcp", () => {
    it("should handle initialization request", async () => {
      const response = await req
        .post("/mcp")
        .set("Accept", "application/json, text/event-stream")
        .send({
          jsonrpc: "2.0",
          method: "initialize",
          params: {
            protocolVersion: "2024-11-05",
            capabilities: {},
            clientInfo: {
              name: "test-client",
              version: "1.0.0",
            },
          },
          id: 1,
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty("jsonrpc", "2.0");
      expect(response.body).toHaveProperty("id", 1);
      expect(response.body.result).toHaveProperty("serverInfo");
      expect(response.body.result.serverInfo.name).toBe("jscpd-server");
    });

    it("should reject requests without session ID after initialization", async () => {
      await req
        .post("/mcp")
        .set("Accept", "application/json, text/event-stream")
        .send({
          jsonrpc: "2.0",
          method: "tools/list",
          id: 2,
        })
        .expect(400);
    });

    it("should return 405 for GET requests", async () => {
      await req.get("/mcp").expect(405);
    });

    it("should return error for invalid JSON", async () => {
      await req
        .post("/mcp")
        .set("Content-Type", "application/json")
        .send("invalid-json")
        .expect(400); // Express likely handles this before our handler
    });
  });
});
