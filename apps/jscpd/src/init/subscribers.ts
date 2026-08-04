import {InFilesDetector, ProgressSubscriber, VerboseSubscriber} from '@jscpd/finder';
import {IOptions} from '@jscpd/core';

// Reporters that print every clone themselves. Streaming the same clones from
// the progress subscriber would print each one twice.
const REPORTERS_PRINTING_CLONES = ['ai', 'consoleFull'];

export function registerSubscribers(options: IOptions, detector: InFilesDetector): void {
  if (options.verbose) {
    detector.registerSubscriber(new VerboseSubscriber(options));
  }

  const reporterPrintsClones = options.reporters?.some((reporter: string) =>
    REPORTERS_PRINTING_CLONES.includes(reporter),
  );

  if (!options.silent && !reporterPrintsClones) {
    detector.registerSubscriber(new ProgressSubscriber(options));
  }
}
