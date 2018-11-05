import { Command } from 'commander';
import { existsSync } from 'fs';
import { readJSONSync } from 'fs-extra';
import { dirname, isAbsolute, resolve } from 'path';
import { IOptions } from '..';
import { getSupportedFormats } from '../tokenizer/formats';

export function getOption(name: string, options?: IOptions): any {
  return options ? (options as any)[name] || (getDefaultOptions() as any)[name] : (getDefaultOptions() as any)[name];
}

export function prepareOptions(cli: Command): IOptions {
  let config: string = cli.config ? resolve(cli.config) : resolve('.jscpd.json');
  let storedConfig: any = {};
  let argsConfig: any;

  argsConfig = {
    minLines: cli.minLines as number,
    maxLines: cli.maxLines as number,
    maxSize: cli.maxSize,
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
    threshold: cli.threshold as number,
    mode: cli.mode,
    absolute: cli.absolute,
    gitignore: cli.gitignore
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

  argsConfig.path = cli.path ? [cli.path].concat(cli.args) : cli.args;

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
  result.listeners = result.listeners || [];

  if (result.silent) {
    result.reporters = result.reporters.filter(reporter => reporter.indexOf('console') === -1).concat('silent');
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
    path: [process.cwd()],
    minLines: 5,
    maxLines: 500,
    maxSize: '30kb',
    minTokens: 50,
    output: './report',
    reporters: ['console', 'time'],
    listeners: ['statistic'],
    ignore: [],
    mode: 'mild',
    threshold: 0,
    format: [...getSupportedFormats()],
    formatsExts: {},
    debug: false,
    silent: false,
    blame: false,
    cache: true,
    absolute: false,
    gitignore: false
  };
}

function parseFormatsExtensions(extensions: string): { [key: string]: string[] } {
  const result: { [key: string]: string[] } = {};

  if (extensions) {
    extensions.split(';').forEach((format: string) => {
      const pair = format.split(':');
      result[pair[0]] = pair[1].split(',');
    });
  }
  return result;
}
