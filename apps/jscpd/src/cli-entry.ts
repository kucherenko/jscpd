import { IOptions } from '@jscpd/core';
import { EntryWithContent, getFilesToDetect } from '@jscpd/finder';
import { Command } from 'commander';
import { initOptionsFromCli, readPackageJson, createBaseCommand, addCommonOptions } from './setup';
import { printFiles, printOptions, printSupportedFormat } from './print';
import { getStore } from './setup/store';
import { detectClones } from './index';

function initCli(packageJson: any, argv: string[]): Command {
	const cli = createBaseCommand(packageJson);

	cli
		.usage('[options] <path ...>')
		.description(packageJson.description)
		.option(
			'-t, --threshold [number]',
			'threshold for duplication, in case duplications >= threshold jscpd will exit with error',
		)
		.option(
			'-r, --reporters [string]',
			'reporters or list of reporters separated with comma to use (Default is time,console)',
		)
		.option('-o, --output [string]', 'reporters to use (Default is ./report/)')
		.option('-p, --pattern [string]', 'glob pattern to file search (Example **/*.txt)')
		.option('-b, --blame', 'blame authors of duplications (get information about authors from git)')
		.option('-s, --silent', 'do not write detection progress and result to a console')
		.option('--store [string]', 'use for define custom store (e.g. --store leveldb used for big codebase)')
		.option('--formats-exts [string]', 'list of formats with file extensions (javascript:es,es6;dart:dt)')
		.option('-d, --debug', 'show debug information, not run detection process(options list and selected files)')
		.option('-v, --verbose', 'show full information during detection process')
		.option('--list', 'show list of total supported formats')
		.option('--exitCode [number]', 'exit code to use when code duplications are detected');

	addCommonOptions(cli);

	cli.parse(argv);
	return cli as Command;
}

export async function runCli(argv: string[], exitCallback?: (code: number) => void): Promise<any[]> {
  const packageJson = readPackageJson();
  const cli = initCli(packageJson, argv);
  const options: IOptions = initOptionsFromCli(cli);

  if (options.list) {
    printSupportedFormat();
  }

  if (options.debug) {
    printOptions(options);
  }

  if (!options.path || options.path.length === 0) {
    options.path = [process.cwd()];
  }

  if (options.debug) {
    const files: EntryWithContent[] = getFilesToDetect(options);
    printFiles(files);
    return Promise.resolve([]);
  }

  const store = getStore(options.store);
  return detectClones(options, store)
    .then((clones) => {
      if (clones.length > 0) {
        exitCallback?.(options.exitCode || 0);
      }
      return clones;
    })
    .finally(async () => {
      await store.close();
    });
}

