import {BlamerHook, FragmentsHook, InFilesDetector} from '@jscpd-ai/finder';
import {IOptions} from '@jscpd-ai/core';

export function registerHooks(options: IOptions, detector: InFilesDetector): void {
  detector.registerHook(new FragmentsHook());
  if (options.blame) {
    detector.registerHook(new BlamerHook());
  }
}
