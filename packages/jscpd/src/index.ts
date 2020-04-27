import {IClone, IOptions, MemoryStore, Statistic} from '@jscpd/core';
import {grey, italic} from 'colors/safe';
import {
	ConsoleFullReporter,
	ConsoleReporter,
	EntryWithContent,
	getFilesToDetect,
	HtmlReporter,
	InFilesDetector,
	JsonReporter,
	ProgressSubscriber,
	SilentReporter,
	ThresholdReporter,
	XmlReporter,
} from '@jscpd/finder';
import {initCli, initOptions} from './init';
import {printFiles, printOptions, printSupportedFormat} from './print';
import {createHash} from "crypto";

export function jscpd(argv: string[]): Promise<IClone[]> {
	const packageJson = require(__dirname + '/../package.json');

	console.time(italic(grey('Detection time:')));

	const cli = initCli(packageJson, argv);

	const options: IOptions = initOptions(cli);

	if (options.list) {
		printSupportedFormat();
	}

	if (options.debug) {
		printOptions(options);
	}

	const files: EntryWithContent[] = getFilesToDetect(options);

	if (options.debug) {
		printFiles(files);
	} else {
		const hashFunction = (value: string): string => {
			return createHash('md5').update(value).digest('hex')
		}
		options.hashFunction = options.hashFunction || hashFunction;
		const store = new MemoryStore();
		const statistic = new Statistic(options);
		const detector = new InFilesDetector(options, statistic, store);

		if (!options.silent) {
			detector.registerSubscriber(new ProgressSubscriber(options));
			if (options.reporters.includes('consoleFull')) {
				detector.registerReporter(new ConsoleFullReporter(options));
			} else if (options.reporters.includes('console')) {
				detector.registerReporter(new ConsoleReporter(options));
			}
		} else {
			detector.registerReporter(new SilentReporter());
		}

		if (options.reporters.includes('html')) {
			detector.registerReporter(new HtmlReporter(options));
		}

		if (options.reporters.includes('json')) {
			detector.registerReporter(new JsonReporter(options));
		}

		if (options.reporters.includes('xml')) {
			detector.registerReporter(new XmlReporter(options));
		}

		if (options.threshold !== undefined) {
			detector.registerReporter(new ThresholdReporter(options));
		}

		return detector.detect(files).then((clones: IClone[]) => {
			console.timeEnd(italic(grey('Detection time:')));
			return clones;
		});
	}
}

