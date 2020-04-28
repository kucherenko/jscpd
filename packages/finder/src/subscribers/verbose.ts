import {grey, yellow} from 'colors/safe';
import {DetectorEvents, IEventPayload, IHandler, IOptions, ISubscriber} from '@jscpd/core';

export class VerboseSubscriber implements ISubscriber {

	constructor(protected options: IOptions) {
	}

	subscribe(): Partial<Record<DetectorEvents, IHandler>> {
		return {
      'CLONE_FOUND': (payload: IEventPayload): void => {
        const {clone} = payload;
        console.log(yellow('CLONE_FOUND'));
        console.log(grey(JSON.stringify(clone, null, '\t')));
      },
      'CLONE_SKIPPED': (payload: IEventPayload): void => {
        const {validation} = payload;
        console.log(yellow('CLONE_SKIPPED'));
        console.log(
          grey('Clone skipped: ' + validation.message.join(' ')),
        );
      },
      'START_DETECTION': (payload: IEventPayload): void => {
        const {source} = payload;
        console.log(yellow('START_DETECTION'));
        console.log(
          grey('Start detection for source id=' + source.getId() + ' format=' + source.getFormat()),
        )
      },
    };
	}
}
