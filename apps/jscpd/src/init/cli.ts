import {Command} from 'commander';
import {getOption} from '@jscpd/core';

export function initCli(packageJson: any, argv: string[]): Command {
	const cli = new Command(packageJson.name);

	cli.version(packageJson.version)
		.usage('[options] <path ...>')
		.description(packageJson.description)
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
			'-t, --threshold [number]',
			'threshold for duplication, in case duplications >= threshold jscpd will exit with error',
		)
		.option('-c, --config [string]', 'path to config file (Default is .jscpd.json in <path>)')
		.option('-i, --ignore [string]', 'glob pattern for files what should be excluded from duplication detection')
		.option('--ignore-pattern [string]', 'Ignore code blocks matching the regexp patterns')
		.option(
			'-r, --reporters [string]',
			'reporters or list of reporters separated with comma to use (Default is time,console)',
		)
		.option('-o, --output [string]', 'reporters to use (Default is ./report/)')
		.option(
			'-m, --mode [string]',
			'mode of quality of search, can be "strict", "mild" and "weak" (Default is "' + getOption('mode') + '")',
		)
		.option('-f, --format [string]', 'format or formats separated by comma (Example php,javascript,python)')
		.option('-p, --pattern [string]', 'glob pattern to file search (Example **/*.txt)')
		.option('-b, --blame', 'blame authors of duplications (get information about authors from git)')
		.option('-s, --silent', 'do not write detection progress and result to a console')
		.option('--store [string]', 'use for define custom store (e.g. --store leveldb used for big codebase)')
		.option('--store-path [string]', 'directory to use for store cache (e.g. --store-path /tmp/jscpd-cache, useful when running multiple instances in parallel)')
		.option('-a, --absolute', 'use absolute path in reports')
		.option('-n, --noSymlinks', 'dont use symlinks for detection in files')
		.option('--ignoreCase', 'ignore case of symbols in code (experimental)')
		.option('-g, --gitignore', 'respect .gitignore files (default: enabled, use --no-gitignore to disable)')
		.option('--no-gitignore', 'do not respect .gitignore files')
		.option('--formats-exts [string]', 'list of formats with file extensions (javascript:es,es6;dart:dt)')
		.option('--formats-names [string]', 'list of formats with specific filenames (makefile:Makefile,GNUmakefile;docker:Dockerfile)')
		.option('-d, --debug', 'show debug information, not run detection process(options list and selected files)')
		.option('-v, --verbose', 'show full information during detection process')
		.option('--list', 'show list of total supported formats')
		.option('--skipLocal', 'skip duplicates in local folders, just detect cross folders duplications')
    .option('--exitCode [number]', 'exit code to use when code duplications are detected')
    .option('--noTips', 'do not print tips and promotional messages after detection')
    .option('--skipComments', 'ignore comments during detection (alias for --mode weak)')

	cli.allowExcessArguments(true);
	cli.parse(argv);
	return cli as Command;
}
