import { IOptions } from '@jscpd/core';
import { Command } from 'commander';
import { initOptionsFromCli, readPackageJson, createBaseCommand, addCommonOptions, getWorkingDirectory } from './setup';

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

export async function runServer(argv: string[], exitCallback?: (code: number) => {}): Promise<any[]> {
  const packageJson = readPackageJson();
  const filteredArgv = argv.filter((arg, index) => !(arg === 'server' && index > 1));
  const cli = initServerCli(packageJson, filteredArgv);
  const options: IOptions = initOptionsFromCli(cli);

  const serverOpts = cli.opts();
  const workingDirectory = getWorkingDirectory(cli);

  try {
    const { startServer } = await import('./server');
    await startServer(workingDirectory, {
      port: serverOpts.port ? parseInt(serverOpts.port, 10) : undefined,
      host: serverOpts.host,
      jscpdOptions: options,
    });
    return Promise.resolve([]);
  } catch (error) {
    console.error('Failed to start server:', error);
    exitCallback?.(1);
    return Promise.resolve([]);
  }
}

