import { Command } from 'commander';
import { createHash } from 'crypto';
import { existsSync } from 'fs';
import { readJSONSync } from 'fs-extra';
import { dirname, isAbsolute, resolve } from 'path';
import { getSupportedFormats } from '../formats';
import { IOptions } from '../interfaces/options.interface';
import { ISource } from '../interfaces/source.interface';

export function generateHashForSource(source: ISource): string {
  return md5(source.id + source.source).substr(0, 10);
}

export function md5(value: string): string {
  return createHash('md5')
    .update(value)
    .digest('hex');
}

export function prepareOptions(cli: Command): IOptions {
  let config: string = cli.config ? resolve(cli.config) : resolve('.cpd.json');
  let storedConfig: any = {};
  let argsConfig: any;

  argsConfig = {
    minLines: cli['min-lines'],
    debug: cli.debug,
    silent: cli.silent,
    blame: cli.blame,
    cache: cli.cache,
    output: cli.output,
    format: cli.format,
    list: cli.list,
    threshold: cli.threshold,
    mode: cli.mode
  };

  if (cli.reporter) {
    argsConfig.reporter = cli.reporter.split(',');
  }

  if (cli.format) {
    argsConfig.format = cli.format.split(',');
  }

  if (cli.ignore) {
    argsConfig.ignore = cli.ignore.split(',');
  }

  if (cli.args[0]) {
    argsConfig.path = cli.args[0] || cli.path;
  }

  Object.keys(argsConfig).forEach(key => {
    if (typeof argsConfig[key] === 'undefined') {
      delete argsConfig[key];
    }
  });

  if (!existsSync(config)) {
    config = '';
  } else {
    storedConfig = readJSONSync(config);
    if (storedConfig.hasOwnProperty('path') && !isAbsolute(storedConfig.path)) {
      storedConfig.path = resolve(dirname(config), storedConfig.path);
    }
  }

  const result: IOptions = {
    ...{ config },
    ...getDefaultOptions(),
    ...storedConfig,
    ...argsConfig
  };

  if (result.silent) {
    result.reporter = ['silent'];
  }
  result.reporter = ['stat', ...(result.reporter || []), 'time'];
  result.reporter = [...new Set(result.reporter)];
  return result;
}

export function getDefaultOptions(): IOptions {
  return {
    executionId: new Date().toISOString(),
    path: process.cwd(),
    minLines: 5,
    minTokens: 50,
    output: './report',
    reporter: ['console'],
    ignore: [],
    mode: 'mild',
    threshold: 0,
    format: [...getSupportedFormats()],
    debug: false,
    silent: false,
    blame: false,
    cache: true
  };
}
