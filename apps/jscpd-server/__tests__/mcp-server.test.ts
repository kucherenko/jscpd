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
  let sessionId: string;

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
      
      // Extract session ID from headers
      sessionId = response.headers["mcp-session-id"] as string;
      expect(sessionId).toBeDefined();
    });

    it("should handle check_duplication tool with auto-recheck", async () => {
      const response = await req
        .post("/mcp")
        .set("Accept", "application/json, text/event-stream")
        .set("mcp-session-id", sessionId)
        .send({
          jsonrpc: "2.0",
          method: "tools/call",
          params: {
            name: "check_duplication",
            arguments: {
              code: "function test() { console.log('hello'); }",
              format: "javascript",
              recheck: true,
            },
          },
          id: 2,
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty("jsonrpc", "2.0");
      expect(response.body.result).toHaveProperty("content");
      // The content should be the validation result (no duplication in this case)
      expect(response.body.result.content[0].text).toContain("duplications");
      expect(response.body.result.content[0].text).toContain("totalDuplications");
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
    it("should handle check_current_directory tool", async () => {
      const response = await req
        .post("/mcp")
        .set("Accept", "application/json, text/event-stream")
        .set("mcp-session-id", sessionId)
        .send({
          jsonrpc: "2.0",
          method: "tools/call",
          params: {
            name: "check_current_directory",
            arguments: {},
          },
          id: 3,
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty("jsonrpc", "2.0");
      expect(response.body).toHaveProperty("id", 3);
      expect(response.body.result).toHaveProperty("content");
      
      const content = response.body.result.content[0].text;
      const stats = JSON.parse(content);
      expect(stats).toHaveProperty("statistics");
    });
  });
});
