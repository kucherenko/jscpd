import {BlamerHook, FragmentsHook, InFilesDetector} from '@jscpd/finder';
import {IOptions} from '@jscpd/core';

export function registerHooks(options: IOptions, detector: InFilesDetector): void {
  detector.registerHook(new FragmentsHook());
  if (options.blame) {
    detector.registerHook(new BlamerHook());
  }
}
