import { IOptions } from '@jscpd/core';
import { Command } from 'commander';
import { initOptionsFromCli, readPackageJson, createBaseCommand, addCommonOptions, getWorkingDirectory } from './setup';
import type { JscpdServer } from './server/server';

function initServerCli(packageJson: any, argv: string[]): Command {
  const cli = createBaseCommand(packageJson);

  cli
    .usage('server [options] <path>')
    .description('Start jscpd as a server')
    .option('-p, --port [number]', 'port to run the server on (Default is 3000)')
    .option('--host [string]', 'host to bind the server to (Default is 0.0.0.0)');

  addCommonOptions(cli);

  cli.parse(argv);

  return cli as Command;
}

export async function runServer(argv: string[], exitCallback?: (code: number) => {}): Promise<JscpdServer | null> {
  const packageJson = readPackageJson();
  // Filter out 'server' from argv before passing to commander
  // Commander expects: command [options] <path>
  const filteredArgv = argv.filter((arg, index) => !(arg === 'server' && index > 1));
  const cli = initServerCli(packageJson, filteredArgv);
  const options: IOptions = initOptionsFromCli(cli);

  const serverOpts = cli.opts();
  const workingDirectory = getWorkingDirectory(cli);

  try {
    const { startServer } = await import('./server');
    const port = serverOpts.port ? parseInt(serverOpts.port, 10) : undefined;
    if (port !== undefined && (isNaN(port) || port < 1 || port > 65535)) {
      throw new Error(`Invalid port number: ${serverOpts.port}`);
    }

    const server = await startServer(workingDirectory, {
      port,
      host: serverOpts.host,
      jscpdOptions: options,
    });

    return server;
  } catch (error) {
    console.error('Failed to start server:', error);
    exitCallback?.(1);
    return null;
  }
}

