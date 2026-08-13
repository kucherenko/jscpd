import {IOptions} from '@jscpd/core';
import {InFilesDetector} from '../in-files-detector';
import {ProgressSubscriber} from '../subscribers/progress';
import {VerboseSubscriber} from '../subscribers/verbose';

// Reporters that print every clone themselves. Streaming the same clones from
// the progress subscriber would print each one twice.
const REPORTERS_PRINTING_CLONES = ['ai', 'consoleFull'];

/**
 * Attach the standard verbose/progress subscribers to a detector, skipping
 * the progress announcer when a configured reporter prints every clone
 * itself. Shared by the jscpd CLI and jscpd-server so the double-print
 * exclusion cannot drift between them.
 */
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
