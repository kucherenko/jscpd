import {dirname, resolve, isAbsolute, relative} from "path";
import {existsSync} from "fs";
import {Command} from 'commander';
import {readJSONSync} from 'fs-extra';
import {getDefaultOptions, IOptions} from '@jscpd/core';
import {parseFormatsExtensions, parseFormatsNames} from '@jscpd/finder';

const resolveIgnorePattern = (configDir: string, pattern: string): string => {
  // Don't modify if pattern is already absolute
  if (isAbsolute(pattern)) {
    return pattern;
  }
  // Don't modify if pattern starts with ** (meant to match at any depth)
  if (pattern.startsWith('**/')) {
    return pattern;
  }
  // For relative patterns, we need to adjust them to be relative to cwd
  // instead of the config directory
  const absolutePattern = resolve(configDir, pattern);
  const cwd = process.cwd();
  // If the config is in cwd or a subdirectory of cwd, make pattern relative to cwd
  const relativePath = relative(cwd, absolutePattern);
  if (!relativePath.startsWith('..')) {
    return relativePath;
  }
  // Otherwise return as absolute
  return absolutePattern;
};

const convertCliToOptions = (cli: Command): Partial<IOptions> => {
  const opts = cli.opts();

  // In Commander v8+, options are no longer set as direct properties on the
  // Command instance — use cli.opts() to retrieve them.
  //
  // gitignore: when neither --gitignore nor --no-gitignore is passed,
  // Commander v15 returns undefined (no implicit default when both
  // positive and negative options are defined). Stripping undefined here
  // lets config files and getDefaultOptions() win, which is the desired
  // behaviour.  When an explicit flag is passed the value is true/false.
  const result: Partial<IOptions> = {
    minTokens: opts.minTokens ? parseInt(opts.minTokens) : undefined,
    minLines: opts.minLines ? parseInt(opts.minLines) : undefined,
    maxLines: opts.maxLines ? parseInt(opts.maxLines) : undefined,
    maxSize: opts.maxSize,
    debug: opts.debug,
    store: opts.store,
    storePath: opts.storePath,
    pattern: opts.pattern,
    executionId: opts.executionId,
    silent: opts.silent,
    blame: opts.blame,
    verbose: opts.verbose,
    cache: opts.cache,
    output: opts.output,
    format: opts.format,
    formatsExts: parseFormatsExtensions(opts.formatsExts),
    formatsNames: parseFormatsNames(opts.formatsNames),
    list: opts.list,
    mode: opts.skipComments && !opts.mode ? 'weak' : opts.mode,
    absolute: opts.absolute,
    noSymlinks: opts.noSymlinks,
    skipLocal: opts.skipLocal,
    ignoreCase: opts.ignoreCase,
    gitignore: opts.gitignore,
    exitCode: opts.exitCode,
    noTips: opts.noTips,
  };

  if (opts.threshold !== undefined) {
    result.threshold = Number(opts.threshold);
  }

  if (opts.reporters) {
    result.reporters = opts.reporters.split(',');
  }

  if (opts.format) {
    result.format = opts.format.split(',');
  }
  if (opts.ignore) {
    result.ignore = opts.ignore.split(',');
  }
  if (opts.ignorePattern) {
    result.ignorePattern = opts.ignorePattern.split(',');
  }
  result.path = opts.path ? [opts.path].concat(cli.args) : cli.args;

  if (result.path.length === 0) {
    delete result.path;
  }

  Object.keys(result).forEach((key) => {
    if (typeof result[key as keyof IOptions] === 'undefined') {
      delete result[key as keyof IOptions];
    }
  });

  return result;
}

const readConfigJson = (config: string | undefined): Partial<IOptions> => {
  const configFile: string = config ? resolve(config) : resolve('.jscpd.json');
  const configExists = existsSync(configFile);
  if (configExists) {
    const result = {config: configFile, ...readJSONSync(configFile)};
    const configDir = dirname(configFile);
    if (result.path) {
      result.path = result.path.map((path: string) => resolve(configDir, path));
    }
    if (result.ignore) {
      result.ignore = result.ignore.map((pattern: string) => resolveIgnorePattern(configDir, pattern));
    }
    return result;
  }
  return {};
}

const readPackageJsonConfig = (): Partial<IOptions> => {
  const config = resolve(process.cwd() + '/package.json');
  if (existsSync(config)) {
    let json: Record<string, any>;
    try {
      json = readJSONSync(config);
    } catch (e) {
      console.warn(`Warning: Could not parse ${config}: ${(e as Error).message}`);
      return {};
    }
    const configDir = dirname(config);
    if (json.jscpd && json.jscpd.path) {
      json.jscpd.path = json.jscpd.path.map((path: string) => resolve(configDir, path));
    }
    if (json.jscpd && json.jscpd.ignore) {
      json.jscpd.ignore = json.jscpd.ignore.map((pattern: string) => resolveIgnorePattern(configDir, pattern));
    }
    return json.jscpd ? {config, ...json.jscpd} : {};
  }
  return {};
}

export function prepareOptions(cli: Command): IOptions {
  const storedConfig: Partial<IOptions> = readConfigJson(cli.opts().config);
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
