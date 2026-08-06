import { McpServer } from "@modelcontextprotocol/server";
import { z } from "zod";
import { JscpdServerService } from "./service";
import { API_INFO, MCP_CACHE_HINTS } from "./constants";

export const STATISTICS_RESOURCE_URI = "jscpd://statistics";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function jsonContent(payload: unknown, indent = 2) {
  return [
    {
      type: "text" as const,
      text: JSON.stringify(payload, null, indent),
    },
  ];
}

function errorContent(message: string) {
  return {
    isError: true,
    content: [
      {
        type: "text" as const,
        text: message,
      },
    ],
  };
}

/**
 * Builds a fresh server instance for a single serving unit.
 *
 * `createMcpHandler` calls this per request, so an instance must never carry
 * state between exchanges: everything durable lives in {@link JscpdServerService}.
 * The same factory backs the modern (2026-07-28) and the legacy (2025-era)
 * path, which keeps both eras exposing an identical tool and resource surface.
 */
export const createMcpServer = (service: JscpdServerService): McpServer => {
  const server = new McpServer(
    {
      name: API_INFO.NAME,
      version: API_INFO.VERSION,
    },
    {
      capabilities: {
        logging: {},
        tools: {},
        resources: {},
      },
      cacheHints: MCP_CACHE_HINTS,
    },
  );

  server.registerTool(
    "check_duplication",
    {
      title: "Check duplication",
      description: "Check code snippet for duplications against the codebase",
      inputSchema: z.object({
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
          .describe(
            "Trigger a re-scan of the current working directory before checking",
          ),
      }),
    },
    async ({ code, format, recheck }) => {
      try {
        if (recheck) {
          await service.recheck();
        }
        const result = await service.checkSnippet({ code, format });
        return { content: jsonContent(result) };
      } catch (error: unknown) {
        return errorContent(
          `Error checking duplication: ${errorMessage(error)}`,
        );
      }
    },
  );

  server.registerTool(
    "get_statistics",
    {
      title: "Get statistics",
      description: "Get overall project duplication statistics",
    },
    () => {
      try {
        return { content: jsonContent(service.getStatistics()) };
      } catch (error: unknown) {
        return errorContent(`Error getting statistics: ${errorMessage(error)}`);
      }
    },
  );

  server.registerTool(
    "check_current_directory",
    {
      title: "Check current directory",
      description:
        "Trigger a re-scan of the current working directory for duplications",
    },
    async () => {
      try {
        await service.recheck();
        return { content: jsonContent(service.getStatistics(), 0) };
      } catch (error: unknown) {
        return errorContent(`Error starting recheck: ${errorMessage(error)}`);
      }
    },
  );

  server.registerResource(
    "statistics",
    STATISTICS_RESOURCE_URI,
    {
      description: "Get overall project duplication statistics",
      mimeType: "application/json",
      cacheHint: MCP_CACHE_HINTS["resources/read"],
    },
    (uri) => {
      try {
        return {
          contents: [
            {
              uri: uri.href,
              text: JSON.stringify(service.getStatistics(), null, 2),
            },
          ],
        };
      } catch (error: unknown) {
        throw new Error(
          `Error getting statistics resource: ${errorMessage(error)}`,
        );
      }
    },
  );

  return server;
};
