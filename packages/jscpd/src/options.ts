import {dirname, resolve} from "path";
import {existsSync} from "fs";
import {Command} from 'commander';
import {readJSONSync} from 'fs-extra';
import {getDefaultOptions, IOptions} from '@jscpd/core';
import {parseFormatsExtensions} from '@jscpd/finder';

const convertCliToOptions = (cli: Command): Partial<IOptions> => {
  const result: Partial<IOptions> = {
    minTokens: cli.minTokens ? parseInt(cli.minTokens) : undefined,
    minLines: cli.minLines ? parseInt(cli.minLines) : undefined,
    maxLines: cli.maxLines ? parseInt(cli.maxLines) : undefined,
    maxSize: cli.maxSize,
    debug: cli.debug,
    store: cli.store,
    pattern: cli.pattern,
    executionId: cli.executionId,
    silent: cli.silent,
    blame: cli.blame,
    verbose: cli.verbose,
    cache: cli.cache,
    output: cli.output,
    format: cli.format,
    formatsExts: parseFormatsExtensions(cli.formatsExts),
    list: cli.list,
    mode: cli.mode,
    absolute: cli.absolute,
    noSymlinks: cli.noSymlinks,
    skipLocal: cli.skipLocal,
    ignoreCase: cli.ignoreCase,
    gitignore: cli.gitignore,
  };

  if (cli.threshold !== undefined) {
    result.threshold = Number(cli.threshold);
  }

  if (cli.reporters) {
    result.reporters = cli.reporters.split(',');
  }

  if (cli.format) {
    result.format = cli.format.split(',');
  }
  if (cli.ignore) {
    result.ignore = cli.ignore.split(',');
  }
  result.path = cli.path ? [cli.path].concat(cli.args) : cli.args;

  if (result.path.length === 0) {
    delete result.path;
  }

  Object.keys(result).forEach((key) => {
    if (typeof result[key] === 'undefined') {
      delete result[key];
    }
  });

  return result;
}

const readConfigJson = (config: string | undefined): Partial<IOptions> => {
  const configFile: string = config ? resolve(config) : resolve('.jscpd.json');
  const configExists = existsSync(configFile);
  if (configExists) {
    const result = {config: configFile, ...readJSONSync(configFile)};
    if (result.path) {
      result.path = result.path.map((path: string) => resolve(dirname(configFile), path));
    }
    return result;
  }
  return {};
}

const readPackageJsonConfig = (): Partial<IOptions> => {
  const config = resolve(process.cwd() + '/package.json');
  if (existsSync(config)) {
    const json = readJSONSync(config);
    if (json.jscpd && json.jscpd.path) {
      json.jscpd.path = json.jscpd.path.map((path: string) => resolve(dirname(config), path));
    }
    return json.jscpd ? {config, ...json.jscpd} : {};
  }
  return {};
}

export function prepareOptions(cli: Command): IOptions {
  const storedConfig: Partial<IOptions> = readConfigJson(cli.config);
  const packageJsonConfig: Partial<IOptions> = readPackageJsonConfig();

  const argsConfig: Partial<IOptions> = convertCliToOptions(cli);

  const result: IOptions = {
    ...getDefaultOptions(),
    ...packageJsonConfig,
    ...storedConfig,
    ...argsConfig,
  };

  result.reporters = result.reporters || [];
  result.listeners = result.listeners || [];

  if (result.silent) {
    result.reporters = result.reporters
      .filter(
        (reporter) => !reporter.includes('console'),
      )
      .concat('silent');
  }

  if (result.threshold !== undefined) {
    result.reporters = [...result.reporters, 'threshold'];
  }

  return result;
}
