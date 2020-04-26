import {grey, red} from 'colors/safe';
import {DetectorEvents, IEventPayload, IHandler, IOptions, ISubscriber} from '@jscpd/core';

export class VerboseSubscriber implements ISubscriber {
	private startTime: [number, number];
	private sourceCount: number = 0;

	constructor(protected options: IOptions) {
		this.startTime = process.hrtime();
	}

	subscribe(): Partial<Record<DetectorEvents, IHandler>> {
		return {
			'CLONE_FOUND': (payload: IEventPayload) => {
				const {clone} = payload;
				console.log(red('Clone found:'));
				console.log(grey(JSON.stringify(clone, null, '\t')));
			},
			'CLONE_SKIPPED': (payload: IEventPayload) => {
				const {validation} = payload;
				console.log(
					grey('Clone skipped: ' + validation.message.join(' ')),
				);
			},
			'START_DETECTION': (payload: IEventPayload) => {
				const {source} = payload;
				console.log(grey('Start detection for source id=' + source.getId() + ' format=' + source.getFormat()))
			},
		};
	}
}
