import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { JscpdServerService } from "./service";
import { API_INFO } from "./constants";

export const createMcpServer = (service: JscpdServerService) => {
  const server = new McpServer(
    {
      name: API_INFO.NAME,
      version: API_INFO.VERSION,
    },
    {
      capabilities: {
        logging: {},
        tools: {},
      },
    },
  );

  server.registerTool(
    "check_duplication",
    {
      description: "Check code snippet for duplications against the codebase",
      inputSchema: {
        code: z
          .string()
          .describe("Source code snippet to check for duplications"),
        format: z
          .string()
          .describe(
            'Format of the code (e.g., "javascript", "typescript", "python")',
          ),
      },
    },
    async ({ code, format }: { code: string; format: string }) => {
      try {
        const result = await service.checkSnippet({ code, format });
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      } catch (error: any) {
        return {
          isError: true,
          content: [
            {
              type: "text",
              text: `Error checking duplication: ${error.message}`,
            },
          ],
        };
      }
    },
  );

  server.registerTool(
    "get_statistics",
    {
      description: "Get overall project duplication statistics",
      inputSchema: {
        // No input required, but providing empty schema for consistency
      },
    },
    async () => {
      try {
        const stats = service.getStatistics();
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(stats, null, 2),
            },
          ],
        };
      } catch (error: any) {
        return {
          isError: true,
          content: [
            {
              type: "text",
              text: `Error getting statistics: ${error.message}`,
            },
          ],
        };
      }
    },
  );

  return server;
};
