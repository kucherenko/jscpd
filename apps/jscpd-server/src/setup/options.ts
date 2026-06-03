import {dirname, resolve} from "path";
import {existsSync} from "fs";
import {Command} from 'commander';
import {readJSONSync} from 'fs-extra';
import {getDefaultOptions, getModeHandler, IOptions} from '@jscpd/core';
import {parseFormatsExtensions} from '@jscpd/finder';
import {getSupportedFormats} from '@jscpd/tokenizer';
import {initIgnore} from './ignore';

const convertCliToOptions = (cli: Command): Partial<IOptions> => {
  const opts = cli.opts();

  const result: Partial<IOptions> = {
    minTokens: opts.minTokens ? parseInt(opts.minTokens) : undefined,
    minLines: opts.minLines ? parseInt(opts.minLines) : undefined,
    maxLines: opts.maxLines ? parseInt(opts.maxLines) : undefined,
    maxSize: opts.maxSize,
    debug: opts.debug,
    store: opts.store,
    pattern: opts.pattern,
    executionId: opts.executionId,
    silent: opts.silent,
    blame: opts.blame,
    verbose: opts.verbose,
    cache: opts.cache,
    output: opts.output,
    format: opts.format,
    formatsExts: parseFormatsExtensions(opts.formatsExts),
    list: opts.list,
    mode: opts.mode,
    absolute: opts.absolute,
    noSymlinks: opts.noSymlinks,
    skipLocal: opts.skipLocal,
    ignoreCase: opts.ignoreCase,
    gitignore: opts.gitignore,
    exitCode: opts.exitCode,
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
    let json: Record<string, any>;
    try {
      json = readJSONSync(config);
    } catch (e) {
      console.warn(`Warning: Could not parse ${config}: ${(e as Error).message}`);
      return {};
    }
    if (json.jscpd && json.jscpd.path) {
      json.jscpd.path = json.jscpd.path.map((path: string) => resolve(dirname(config), path));
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

export function initOptionsFromCli(cli: Command): IOptions {
	const options: IOptions = prepareOptions(cli);

	options.format = options.format || getSupportedFormats();

	options.mode = getModeHandler(options.mode);

	options.ignore = initIgnore(options);

	return options;
}
