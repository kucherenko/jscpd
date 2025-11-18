import { Command } from 'commander';
import { readJSONSync } from 'fs-extra';
import { getOption } from '@jscpd/core';

export const readPackageJson = (): any => {
  return readJSONSync(__dirname + '/../../package.json');
};

export const createBaseCommand = (packageJson: any) => {
  return new Command(packageJson.name).version(packageJson.version);
};

export const addCommonOptions = (cli: any): void => {
  cli
    .option('-c, --config [string]', 'path to config file (Default is .jscpd.json in <path>)')
    .option('-f, --format [string]', 'format or formats separated by comma')
    .option('-i, --ignore [string]', 'glob pattern for files to exclude')
    .option('--ignore-pattern [string]', 'ignore code blocks matching regexp patterns')
    .option(
      '-l, --min-lines [number]',
      'min size of duplication in code lines (Default is ' + getOption('minLines') + ')',
    )
    .option(
      '-k, --min-tokens [number]',
      'min size of duplication in code tokens (Default is ' + getOption('minTokens') + ')',
    )
    .option('-x, --max-lines [number]', 'max size of source in lines (Default is ' + getOption('maxLines') + ')')
    .option(
      '-z, --max-size [string]',
      'max size of source in bytes, examples: 1kb, 1mb, 120kb (Default is ' + getOption('maxSize') + ')',
    )
    .option(
      '-m, --mode [string]',
      'mode of quality of search, can be "strict", "mild" and "weak" (Default is "' + getOption('mode') + '")',
    )
    .option('-a, --absolute', 'use absolute path in reports')
    .option('-n, --noSymlinks', 'dont use symlinks for detection')
    .option('--ignoreCase', 'ignore case of symbols in code (experimental)')
    .option('-g, --gitignore', 'ignore all files from .gitignore file')
    .option('--skipLocal', 'skip duplicates in local folders');
};

export const getWorkingDirectory = (cli: any): string => {
  return cli.args[0] || process.cwd();
};

