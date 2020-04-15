import {Command} from 'commander';
import {prepareOptions} from './options';
import {
	DetectorEvents,
	DuplicationDetector,
	getModeHandler,
	getOption,
	IClone,
	IOptions,
	ReporterHandler,
} from '@jscpd/core';
import {MemoryStore} from '@jscpd/store';
import {bold, grey, italic, white} from 'colors/safe';
import {getFormatByFile, getSupportedFormats} from '@jscpd/tokenizer';
import {Entry, sync} from 'fast-glob';
import {existsSync} from "fs";
import {readFileSync} from 'fs-extra';
import {isFile} from './utils/fs';
import {ConsoleReporter} from './reporters/console';
import bytes = require('bytes');

const gitignoreToGlob = require('gitignore-to-glob');

const packageJson = require(__dirname + '/../package.json');

console.time(italic(grey('Detection time:')));

export const cli = new Command(packageJson.name);

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
	.option('-c, --config [string]', 'path to config file (Default is .cpd.json in <path>)')
	.option('-i, --ignore [string]', 'glob pattern for files what should be excluded from duplication detection')
	.option(
		'-r, --reporters [string]',
		'reporters or list of reporters separated with coma to use (Default is time,console)',
	)
	.option('-o, --output [string]', 'reporters to use (Default is ./report/)')
	.option(
		'-m, --mode [string]',
		'mode of quality of search, can be "strict", "mild" and "weak" (Default is "' + getOption('mode') + '")',
	)
	.option('-f, --format [string]', 'format or formats separated by coma (Example php,javascript,python)')
	.option('-b, --blame', 'blame authors of duplications (get information about authors from git)')
	.option('-s, --silent', 'do not write detection progress and result to a console')
	.option('-a, --absolute', 'use absolute path in reports')
	.option('-n, --noSymlinks', 'dont use symlinks for detection in files')
	.option('--ignoreCase', 'ignore case of symbols in code (experimental)')
	.option('-g, --gitignore', 'ignore all files from .gitignore file')
	.option('--formats-exts [string]', 'list of formats with file extensions (javascript:es,es6;dart:dt)')
	.option('-d, --debug', 'show debug information(options list and selected files)')
	.option('--list', 'show list of total supported formats')
	.option('--skipLocal', 'skip duplicates in local folders, just detect cross folders duplications')
	.option('--xsl-href [string]', '(Deprecated) Path to xsl file')
	.option('-p, --path [string]', '(Deprecated) Path to repo, use `jscpd <path>`');

cli.parse(process.argv);

const options: IOptions = prepareOptions(cli as Command);

options.format = options.format || getSupportedFormats();

options.mode = getModeHandler(options.mode);

if (cli.list) {
	console.log(bold(white('Supported formats: ')));
	console.log(getSupportedFormats().join(', '));
	process.exit(0);
}

if (cli.debug) {
	console.log(bold(white('Options:')));
	console.dir(options);
}

const ignore: string[] = options.ignore || [];

if (options.gitignore && existsSync(process.cwd() + '/.gitignore')) {
	let gitignorePatterns: string[] = gitignoreToGlob(process.cwd() + '/.gitignore') || [];
	gitignorePatterns = gitignorePatterns.map((pattern) =>
		pattern.substr(pattern.length - 1) === '/' ? `${pattern}**/*` : pattern,
	);
	ignore.push(...gitignorePatterns);
	ignore.map((pattern) => pattern.replace('!', ''));
}

let files = sync(
	options.path.map((path: string) => {
		if (isFile(path)) {
			return path;
		}
		return path.substr(path.length - 1) === '/' ? `${path}**/*` : `${path}/**/*`;
	}),
	{
		ignore,
		onlyFiles: true,
		dot: true,
		stats: true,
		absolute: options.absolute,
		followSymbolicLinks: !options.noSymlinks,
	},
).filter((stats: any) => {
	const {path} = stats;
	const format: string = getFormatByFile(path, options.formatsExts) as string;
	return format && options.format && options.format.includes(format);
}).filter((stats: any) => {
	if (options.debug && stats.size > bytes(getOption('maxSize', options))) {
		console.log(`File ${stats.path} skipped! Size more then limit (${bytes(stats.size)} > ${getOption('maxSize', options)})`);
	}
	return !(stats.size > bytes(getOption('maxSize', options)));
}).map((entry: Entry) => {
	const {path} = entry;
	const content = readFileSync(path).toString();
	return {...entry, content}
}).filter((stats: any) => {
	const {path, content} = stats;
	const lines = content.split('\n').length;
	const minLines = getOption('minLines', options);
	const maxLines = getOption('maxLines', options);
	if (lines < minLines || lines > maxLines) {
		if (options.debug) {
			console.log(grey(`File ${path} skipped! Code lines=${lines} not in limits (${minLines}:${maxLines})`));
		}
		return false;
	}
	return true;
});

if (options.debug) {
	files.forEach((stats: Entry) => {
		console.log(grey(stats.path));
	});
	console.log(bold(`Found ${files.length} files to detect.`));
} else {
	(async () => {
		const store = new MemoryStore();
		const detector = new DuplicationDetector(options, store);

		const consoleReporter = new ConsoleReporter(options);

		Object
			.entries(consoleReporter.subscribe())
			.map(([event, handler]: [DetectorEvents, ReporterHandler]) => detector.on(event, handler));

		const clones: IClone[][] = await Promise.all(
			files.map(async (entry: any) => {
				const {path, content} = entry;
				const format: string = getFormatByFile(path, options.formatsExts) as string;
				return await detector.detect(path, content, format);
			}),
		);

		consoleReporter.report(
			clones.reduce((acc: IClone[], item: IClone[]) => {
				if (item.length > 0) {
					acc.push(...item);
				}
				return acc;
			}, []),
		);

		console.timeEnd(italic(grey('Detection time:')));
	})();
}



