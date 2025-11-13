import {InFilesDetector, ProgressSubscriber, VerboseSubscriber} from '@jscpd-ai/finder';
import {IOptions} from '@jscpd-ai/core';

export function registerSubscribers(options: IOptions, detector: InFilesDetector): void {
  if (options.verbose) {
    detector.registerSubscriber(new VerboseSubscriber(options));
  }

  if (!options.silent) {
    detector.registerSubscriber(new ProgressSubscriber(options));
  }
}
