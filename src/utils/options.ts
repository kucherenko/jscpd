import { Command } from 'commander';
import { existsSync } from 'fs';
import { readJSONSync } from 'fs-extra';
import { dirname, isAbsolute, resolve } from 'path';
import { IOptions } from '..';
import { getSupportedFormats } from '../formats';

export function prepareOptions(cli: Command): IOptions {
  let config: string = cli.config ? resolve(cli.config) : resolve('.cpd.json');
  let storedConfig: any = {};
  let argsConfig: any;

  argsConfig = {
    minLines: cli['min-lines'],
    debug: cli.debug,
    executionId: cli.executionId,
    silent: cli.silent,
    blame: cli.blame,
    cache: cli.cache,
    output: cli.output,
    xslHref: cli.xslHref,
    format: cli.format,
    formatsExts: parseFormatsExtensions(cli.formatsExts),
    list: cli.list,
    threshold: cli.threshold,
    mode: cli.mode
  };

  if (cli.reporters) {
    argsConfig.reporters = cli.reporters.split(',');
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

  result.reporters = result.reporters || [];

  if (result.silent) {
    result.reporters = result.reporters
      .filter(reporter => reporter.indexOf('console') === -1)
      .concat('silent');
  }

  if (result.threshold) {
    result.reporters = [...result.reporters, 'threshold'];
  }
  result.reporters = [...result.reporters, 'time'];
  result.reporters = [...new Set(result.reporters)];
  return result;
}

export function getDefaultOptions(): IOptions {
  return {
    executionId: new Date().toISOString(),
    path: process.cwd(),
    minLines: 5,
    minTokens: 50,
    output: './report',
    reporters: ['console', 'time'],
    listeners: ['state', 'statistic'],
    ignore: [],
    mode: 'mild',
    threshold: 0,
    format: [...getSupportedFormats()],
    formatsExts: {},
    debug: false,
    silent: false,
    blame: false,
    cache: true
  };
}

function parseFormatsExtensions(
  extensions: string
): { [key: string]: string[] } {
  const result: { [key: string]: string[] } = {};

  if (extensions) {
    extensions.split(';').forEach((format: string) => {
      const pair = format.split(':');
      result[pair[0]] = pair[1].split(',');
    });
  }
  return result;
}
