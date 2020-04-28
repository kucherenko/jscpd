import {IClone, IOptions, IStore, Statistic} from '@jscpd/core';
import {grey, italic} from 'colors/safe';
import {EntryWithContent, getFilesToDetect, InFilesDetector, ProgressSubscriber} from '@jscpd/finder';
import {initCli, initOptions} from './init';
import {printFiles, printOptions, printSupportedFormat} from './print';
import {createHash} from "crypto";
import {getStore} from './init/store';
import {IMapFrame} from '@jscpd/tokenizer';
import {registerReporters} from './init/reporters';

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
    const store: IStore<IMapFrame> = getStore(cli.store);
    const statistic = new Statistic(options);
    const detector = new InFilesDetector(options, statistic, store);

    if (!options.silent) {
      detector.registerSubscriber(new ProgressSubscriber(options));
    }

    registerReporters(options, detector);

    return detector.detect(files).then((clones: IClone[]) => {
      console.timeEnd(italic(grey('Detection time:')));
      return clones;
    }).finally(() => {
      store.close();
    });
  }
}

