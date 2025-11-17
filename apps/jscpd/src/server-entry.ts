import { IOptions } from '@jscpd/core';
import { Command } from 'commander';
import { readJSONSync } from 'fs-extra';
import { initOptionsFromCli } from './init';

function initServerCli(packageJson: any, argv: string[]): Command {
  const cli = new Command(packageJson.name);

  cli
    .version(packageJson.version)
    .usage('server [options] <path>')
    .description('Start jscpd as a server')
    .option('-p, --port [number]', 'port to run the server on (Default is 3000)')
    .option('--host [string]', 'host to bind the server to (Default is 0.0.0.0)')
    .option('-c, --config [string]', 'path to config file (Default is .jscpd.json in <path>)')
    .option('-f, --format [string]', 'format or formats separated by comma')
    .option('-i, --ignore [string]', 'glob pattern for files to exclude')
    .option('--ignore-pattern [string]', 'ignore code blocks matching regexp patterns')
    .option('-l, --min-lines [number]', 'min size of duplication in code lines')
    .option('-k, --min-tokens [number]', 'min size of duplication in code tokens')
    .option('-x, --max-lines [number]', 'max size of source in lines')
    .option('-z, --max-size [string]', 'max size of source in bytes')
    .option('-m, --mode [string]', 'mode of quality of search (strict, mild, weak)')
    .option('-a, --absolute', 'use absolute path in reports')
    .option('-n, --noSymlinks', 'dont use symlinks for detection')
    .option('--ignoreCase', 'ignore case of symbols in code')
    .option('-g, --gitignore', 'ignore all files from .gitignore file')
    .option('--skipLocal', 'skip duplicates in local folders')
    .parse(argv);

  return cli;
}

/**
 * Run jscpd in server mode for on-demand duplicate detection
 */
export async function runServer(argv: string[], exitCallback?: (code: number) => {}): Promise<any[]> {
  const packageJson = readJSONSync(__dirname + '/../package.json');
  const cli = initServerCli(packageJson, argv);
  const options: IOptions = initOptionsFromCli(cli);

  // Extract server-specific options
  const serverOpts = cli.opts();
  const workingDirectory = cli.args[0] || process.cwd();

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

