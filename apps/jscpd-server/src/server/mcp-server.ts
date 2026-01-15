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
        recheck: z
          .boolean()
          .optional()
          .describe("Trigger a re-scan of the current working directory before checking"),
      },
    },
    async ({ code, format, recheck }: { code: string; format: string; recheck?: boolean }) => {
      try {
        if (recheck) {
          await service.recheck();
        }
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

  server.registerTool(
    "check_current_directory",
    {
      description: "Trigger a re-scan of the current working directory for duplications",
      inputSchema: {
        // No input required
      },
    },
    async () => {
      try {
        await service.recheck();
        const statistics = await service.getStatistics();
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(statistics),
            },
          ],
        };
      } catch (error: any) {
        return {
          isError: true,
          content: [
            {
              type: "text",
              text: `Error starting recheck: ${error.message}`,
            },
          ],
        };
      }
    },
  );

  server.registerResource(
    "statistics",
    "jscpd://statistics",
    {
       description: "Get overall project duplication statistics",
       mimeType: "application/json"
    },
    async (uri) => {
      try {
        const stats = await service.getStatistics();
        return {
          contents: [
            {
              uri: uri.href,
              text: JSON.stringify(stats, null, 2),
            },
          ],
        };
      } catch (error: any) {
         throw new Error(`Error getting statistics resource: ${error.message}`);
      }
    },
  );

  return server;
};
