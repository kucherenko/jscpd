import { IOptions } from "@jscpd/core";
import { configureColors } from "@jscpd/finder";
import { Command, CommanderError } from "commander";
import {
  initOptionsFromCli,
  readPackageJson,
  createBaseCommand,
  addCommonOptions,
  getWorkingDirectory,
} from "./setup";
import type { JscpdServer } from "./server/server";

function collectHostname(value: string, previous: string[]): string[] {
  return [...previous, value];
}

function initServerCli(packageJson: any, argv: string[]): Command {
  const cli = createBaseCommand(packageJson);

  cli.exitOverride();

  cli
    .usage("[options] <path>")
    .description("Start jscpd as a server")
    .helpOption("--help", "display help for command")
    .option(
      "-p, --port [number]",
      "port to run the server on (Default is 3000)",
    )
    .option(
      "-H, --host [string]",
      "host to bind the server to (Default is 127.0.0.1)",
    )
    .option(
      "--allowed-origin <hostname>",
      "extra Origin header hostname accepted by the MCP and REST endpoints, repeatable",
      collectHostname,
      [] as string[],
    )
    .option(
      "--allowed-host <hostname>",
      "Host header hostname the MCP and REST endpoints answer on, repeatable",
      collectHostname,
      [] as string[],
    );

  addCommonOptions(cli);

  cli.allowExcessArguments(true);
  cli.parse(argv);

  return cli as Command;
}

export async function runServer(
  argv: string[],
  exitCallback?: (code: number) => void,
): Promise<JscpdServer | null> {
  const packageJson = readPackageJson();

  let cli: Command;
  try {
    cli = initServerCli(packageJson, argv);
  } catch (error) {
    // Commander has already reported the usage problem (or printed help), so
    // only the exit code has to be relayed.
    exitCallback?.(error instanceof CommanderError ? error.exitCode : 1);
    return null;
  }

  const options: IOptions = initOptionsFromCli(cli);

  // Respect NO_COLOR/FORCE_COLOR and TTY detection for server log output,
  // the same way the jscpd CLI does (supervisors and containers capture
  // stdout, where ANSI escapes are noise).
  configureColors({ colors: options.colors });

  const serverOpts = cli.opts();
  const workingDirectory = getWorkingDirectory(cli);

  try {
    const { startServer } = await import("./server");
    const port = serverOpts.port ? parseInt(serverOpts.port, 10) : undefined;
    if (port !== undefined && (isNaN(port) || port < 1 || port > 65535)) {
      throw new Error(`Invalid port number: ${serverOpts.port}`);
    }

    const server = await startServer(workingDirectory, {
      port,
      host: serverOpts.host,
      allowedOrigins: serverOpts.allowedOrigin,
      allowedHosts: serverOpts.allowedHost,
      jscpdOptions: options,
    });

    return server;
  } catch (error) {
    console.error("Failed to start server:", error);
    exitCallback?.(1);
    return null;
  }
}
