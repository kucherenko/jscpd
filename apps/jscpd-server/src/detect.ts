import {getDefaultOptions, IClone, IMapFrame, IOptions, IStore, Statistic} from '@jscpd/core';
import {grey, italic} from 'colors/safe';
import {EntryWithContent, getFilesToDetect, InFilesDetector} from '@jscpd/finder';
import {createHash} from 'crypto';
import {getStore} from './setup/store';
import {getSupportedFormats, Tokenizer} from '@jscpd/tokenizer';
import {registerReporters} from './setup/reporters';
import {registerSubscribers} from './setup/subscribers';
import {registerHooks} from './setup/hooks';

const TIMER_LABEL = 'Detection time:';

export type DetectorContext = {
  options: IOptions;
  store: IStore<IMapFrame>;
  statistic: Statistic;
  tokenizer: Tokenizer;
  detector: InFilesDetector;
  files: EntryWithContent[];
};

export function createBaseDetectorContext(
  opts: Partial<IOptions>,
  providedStore?: IStore<IMapFrame>
): DetectorContext {
  const options = {...getDefaultOptions(), ...opts};

  if (!options.format) {
    options.format = getSupportedFormats();
  }

  if (!options.hashFunction) {
    options.hashFunction = (value: string) => createHash('md5').update(value).digest('hex');
  }

  const store = providedStore || getStore(options.store);
  const files = getFilesToDetect(options as IOptions);
  const statistic = new Statistic();
  const tokenizer = new Tokenizer();
  const detector = new InFilesDetector(tokenizer, store, statistic, options as IOptions);

  return {options: options as IOptions, store, statistic, tokenizer, detector, files};
}

export async function detectClones(
  opts: IOptions,
  store?: IStore<IMapFrame>
): Promise<IClone[]> {
  const context = createBaseDetectorContext(opts, store);

  registerReporters(context.options, context.detector);
  registerSubscribers(context.options, context.detector);
  registerHooks(context.options, context.detector);

  if (context.options.silent) {
    return context.detector.detect(context.files);
  }

  console.time(italic(grey(TIMER_LABEL)));
  const clones = await context.detector.detect(context.files);
  console.timeEnd(italic(grey(TIMER_LABEL)));

  return clones;
}
